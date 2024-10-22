#![allow(unused)]

// GOAL: Turn Sequence of Bytes in TCP packets into IEEE C37.118.2 formatted structs.

// Define structures common to all frames

// Configuration Frames for PDU+PMUs
// Prefix Frame +
// PDCConfigFrame +
// [PMUFrame1, PMUFrame2,...] // Frames can be fragmented with many PMUs
// CHK - Cyclic Redundancy Check // If fragmented, last two bytes of last fragement contain the CHK.
// CRC-CCITT implementation based on IEEE C37.118.2-2011 Appendix B
pub fn calculate_crc(buffer: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in buffer {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[derive(Debug)]
pub struct PrefixFrame2011 {
    pub sync: u16, // Leading byte = AA hex,
    // second byte: Frame type and version
    // Bit7: reserved=0
    // Bits6-4:
    // 000: Data Frame
    // 001: Header Frame,
    // 010: Configuration Frame 1
    // 011: Configuration Frame 2
    // 101: Configuration Frame 3
    // 100: Command Frame
    // Bits 3-0: Version number in binary (1-15)
    // Version 1 (0001) for messages defined in IEEE Std C37.118-2005
    // Version 2 (0010) for messaged defined in IEEE STD C37.118.2-2011
    pub framesize: u16, // Total number of bytes in the frame including CHK
    pub idcode: u16,
    // Data stream id number
    pub soc: u32, // Time stamp in UNIX time base. Range is 136 years, rolls over in 2106 AD. Leap seconds not included.
    pub fracsec: u32, // Fraction of second and time quaility, time of measurement of data frames,
                  //or time of frame transmission for non-data frames
                  // Bits 31-24: Message Time Quality (TODO needs additional bit mapping)
                  // Bits 23-00: FRACSEC, 24 Bit integer, when divided by TIME_BASE yields actual fractional second. FRACSEC used in all
                  // messages to and from a given PMU shall use the same TIME_BASE that is provided in the configuration message from that PMU.
}
impl PrefixFrame2011 {
    pub fn to_hex(&self) -> [u8; 14] {
        let mut result = [0u8; 14];
        result[0..2].copy_from_slice(&self.sync.to_be_bytes());
        result[2..4].copy_from_slice(&self.framesize.to_be_bytes());
        result[4..6].copy_from_slice(&self.idcode.to_be_bytes());
        result[6..10].copy_from_slice(&self.soc.to_be_bytes());
        result[10..14].copy_from_slice(&self.fracsec.to_be_bytes());
        result
    }

    pub fn from_hex(bytes: &[u8; 14]) -> Result<Self, &'static str> {
        if bytes.len() != 14 {
            return Err("Invalid byte array length");
        }
        Ok(PrefixFrame2011 {
            sync: u16::from_be_bytes([bytes[0], bytes[1]]),
            framesize: u16::from_be_bytes([bytes[2], bytes[3]]),
            idcode: u16::from_be_bytes([bytes[4], bytes[5]]),
            soc: u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
            fracsec: u32::from_be_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]),
        })
    }
}

#[derive(Debug)]
pub struct HeaderFrame2011 {
    pub prefix: PrefixFrame2011,
    pub data_source: [u8; 32], // Data source identifier 32 byte ASCII
    pub version: [u8; 4],      // Version of data file or stream 4 byte ASCII
    pub chk: u16,              // CRC-CCITT
}

// Command Dataframe struct based on 2011 standard
// Should have a simple IMPL interface to create the 7 basic commands.
// Skip the custom commands for now.
#[derive(Debug)]
pub struct CommandFrame2011 {
    pub prefix: PrefixFrame2011,
    pub command: u16,              // Command word
    pub extframe: Option<Vec<u8>>, // Optional extended frame data
    pub chk: u16,
}

impl CommandFrame2011 {
    pub fn new_turn_off_transmission(idcode: u16) -> Self {
        Self::new_command(idcode, 1)
    }

    pub fn new_turn_on_transmission(idcode: u16) -> Self {
        Self::new_command(idcode, 2)
    }

    pub fn new_send_header_frame(idcode: u16) -> Self {
        Self::new_command(idcode, 3)
    }

    pub fn new_send_config_frame1(idcode: u16) -> Self {
        Self::new_command(idcode, 4)
    }

    pub fn new_send_config_frame2(idcode: u16) -> Self {
        Self::new_command(idcode, 5)
    }

    pub fn new_send_config_frame3(idcode: u16) -> Self {
        Self::new_command(idcode, 6)
    }

    pub fn new_extended_frame(idcode: u16) -> Self {
        Self::new_command(idcode, 8)
    }

    // TODO, decide whether to fill in the value now,
    // or wait until the client sends over TCP.
    // Second option is most precise.
    // e.g. get time in seconds and fracsec, calc crc, to_hex, send.
    fn new_command(idcode: u16, command: u16) -> Self {
        let prefix = PrefixFrame2011 {
            sync: 0xAA41,  // Command frame sync
            framesize: 18, // Fixed size for basic command frame
            idcode,
            soc: 0,     // To be filled by sender
            fracsec: 0, // To be filled by sender
        };
        CommandFrame2011 {
            prefix,
            command,
            extframe: None,
            chk: 0,
        }
    }
    pub fn to_hex(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&self.prefix.to_hex());
        result.extend_from_slice(&self.command.to_be_bytes());
        if let Some(extframe) = &self.extframe {
            result.extend_from_slice(extframe);
        }
        let crc = calculate_crc(&result);
        result.extend_from_slice(&crc.to_be_bytes());
        result
    }
}

#[derive(Debug)]
pub enum DataFrameType {
    Float(PMUDataFrameFloat2011),
    Int(PMUDataFrameInt2011),
}

#[derive(Debug)]
pub struct DataFrame2011 {
    pub prefix: PrefixFrame2011,
    pub data: Vec<DataFrameType>, // Length of Vec is based on num phasors.
    pub chk: u16,
}
// This frame is repeated for each PMU available.
#[derive(Debug)]
pub struct PMUDataFrameFloat2011 {
    // Header frame above plus the following
    pub stat: u16,         // Bit-mapped flags
    pub phasors: Vec<u64>, // or u64, Phasor Estimates, May be single phase or 3-phase postive, negative or zero sequence.
    // Four or 8 bytes each depending on the fixed 16-bit or floating point format used, as indicated by the FORMATE field.
    // in the configuration frame. The number of values is determined by the PHNMR field in configuration 1,2,3 frames.
    pub freq: u32,        // or u32, 2 or 4 bytes, fixed or floating point.
    pub dfreq: u32,       // or u32, 2 or 4 bytes, fixed or floating point.
    pub analog: Vec<u32>, // or u32, analog data, 2 or 4 bytes per value depending on fixed or floating point format used,
    // as indicated by the format field in configuration 1, 2, and 3 frames.
    // Number of values is determed by the ANNMR in configuration 1,2, and 3 frames.
    pub digital: Vec<u16>, // Digital data, usually representing 16 digital status points (channels).
                           // The number of values is determined by the DGNMR field in configuration 1, 2, and 3 frames.
}

#[derive(Debug)]
pub struct PMUDataFrameInt2011 {
    // Header frame above plus the following
    pub stat: u16,         // Bit-mapped flags
    pub phasors: Vec<u32>, // or u64, Phasor Estimates, May be single phase or 3-phase postive, negative or zero sequence.
    // Four or 8 bytes each depending on the fixed 16-bit or floating point format used, as indicated by the FORMATE field.
    // in the configuration frame. The number of values is determined by the PHNMR field in configuration 1,2,3 frames.
    pub freq: Vec<u16>, // or u32, 2 or 4 bytes, fixed or floating point.
    pub dfreq: u16,     // or u32, 2 or 4 bytes, fixed or floating point.
    pub analog: u16, // or u32, analog data, 2 or 4 bytes per value depending on fixed or floating point format used,
    // as indicated by the format field in configuration 1, 2, and 3 frames.
    // Number of values is determed by the ANNMR in configuration 1,2, and 3 frames.
    pub digital: u16, // Digital data, usually representing 16 digital status points (channels).
                      // The number of values is determined by the DGNMR field in configuration 1, 2, and 3 frames.
}

#[derive(Debug)]
pub struct ConfigurationFrame1and2_2011 {
    pub prefix: PrefixFrame2011,
    pub time_base: u32, // Resolution of
    pub num_pmu: u16,
    // pmu_configs repeated num_pmu times.
    pub pmu_configs: Vec<PMUConfigurationFrame2011>,
    pub data_rate: i16, // Rate of Data Transmission.
    pub chk: u16,
}

// This struct is repeated NUM_PMU times.
// For parsing entire configuration frame, need to take into account num_pmu.
#[derive(Debug)]
pub struct PMUConfigurationFrame2011 {
    pub stn: [u8; 16], // Station Name 16 bytes ASCII
    pub idcode: u16,   // Data source ID number, identifies source of each data block.
    pub format: u16,   // Data format within the data frame
    // 16-bit flag.
    // Bits 15-4: unused
    // Bit 3: 0=Freq/DFREQ 16-bit integer 1=Floating point
    // Bit 2: 0 = analogs 16-bit integer, 1=floating point
    // Bit 1: phasors 16-bit ineger, 1=floating point
    // Bit 0: phasor real and imaginary (rectangular), 1=magnitude and angle (polar)
    pub phnmr: u16,     // Number of phasors - 2 byte integer
    pub annmr: u16,     // Number of analog values -  2 byte integer
    pub dgnmr: u16,     // number of digital status words - 2 byte integer
    pub chnam: Vec<u8>, // Length = 16 x (PHNMR+ANNMR + 16 x DGNMR)
    // Phasor and channel names, 16 bytes for each phasor analog and each digital channel.
    pub phunit: Vec<u32>, // length = 4 x PHNMR, Conversion factor for phasor channels
    pub anunit: Vec<u32>, // length = 4 x ANNMR, Conversion factor for Analog Channels
    pub digunit: Vec<u32>, // length = 4 x DGNMR, Mask words for digital status words
    pub fnom: u16,        // Nominal Frequency code and flags
    pub cfgcnt: u16,      // Configuration change count.
}

// TODO make IMPL to read out chnam into a list of strings.

// Header frame common to both configuration and data frames.
#[derive(Debug)]
pub struct HeaderFrame2024 {
    pub sync: [u8; 2], // Synchronization bytes, using a u8[2] array here since the first and second byte are read separately.
    pub framesize: u16, // Frame size in bytes, Max=65535, TODO build a test for checking against out of range frames.
    pub stream_id: u16, // Data Stream ID, Identifies destination data stream for commands and source stream for other messages.
    pub soc: u32,       // Timestamp - Time since midnight 01-Jan-1970 (UNIX Time)
    pub leap_byte: u8, // Leap second information, Bit6-> 0=Add, 1=Delete, Bit5->1=Leap second occured, Bit4-> leap second pending
    pub fracsec: [u8; 3], // Fractional Part of a seconde multiplied by TIME_BASE? and rounded to nearest integer.
}

#[derive(Debug)]
struct ConfigTailFrame2024 {
    pub stream_data_rate: u16, // Rate of data transmission for the composite frame in stream. (See PMU_DATA_RATE)
    pub wait_time: u16,        // PDC wait time in milliseconds
    pub chk: u16, // CRC-CCITT (Cyclic Redundancy Check - ),uses polynomial x^16 + x^12 + x^5 + 1, TODO need research more.
}

// Additional Data structures for Configuration frames
// Everything in Common Data frame
#[derive(Debug)]
pub struct PDCConfigurationFrame2024 {
    pub cont_idx: u16, // Continuation index for fragmented frames, 0 no fragments, 1 first frag in series, ...
    pub time_base: u32, // Bits 31-24 reserved =0, Bits23-0 24-bit uint, subdivision of the second that FRACSEC is based on.
    pub pdc_name: String, // TODO: should be 1-256 bytes not sure how to parse if we don't know the length before hand.
    pub num_pmu: u16,     // Number of pmus included in the data frame.

    // ---- Repeated PMU Configuration Frames below for each PMU ---//
    pub stream_data_rate: u16, // Rate of data transmission for the composite frame in stream. (See PMU_DATA_RATE)
    pub wait_time: u16,        // PDC wait time in milliseconds
    pub chk: u16,              //
}

// Repeated NUM_PMU times
#[derive(Debug)]
pub struct PMUConfigurationFrame2024 {
    pub pmu_name: String,   // TODO: SHould be 1-256 bytes
    pub pmu_id: u16,        // 1-65534, 0 and 65535 are reserved
    pub pmu_version: u16,   // Bits 15-4 Reserved =0, Bits 3-0 Version Number from the SYNC word.
    pub g_pmu_id: [u32; 4], // Global PMU ID, Uses RFC 4122 big endian byte encoding.
    pub format: u16, // Bits 15-4 Reserved=0, Bit3=FREQ/DFREQ 0=16bity integer, 1=floating point
    // bit2 Analog 0=16bit int, 1 = floating point
    // bit1 Phasor (format) 0=int, 1=floating point
    // bit0 Phasor (encoding) 0=real and imaginary, 1=magnitude and angle (polar)
    pub phnmr: u16,    //Number of phasors
    pub annmr: u16,    //Number of analog values
    pub frnmr: u16,    //Number of frequency signals,
    pub dfdtnmr: u16,  //Number of df/dt signals,
    pub dgnmr: u16,    //Number of digital status words,
    pub chnam: String, //TODO 1-256 bytes, Phasor and channel name, minimum 2 bytes for each phasor, frequency, ROCOF, analog and digital channel.
    // Names are in the same order as they are transmitted. Re-read IEEE standard,
    pub phscale: [u16; 16], // 16xPHNMR, Conversion factor for phasor channels with flags. Magnitude, and angle scalling for phasors with data flags.
    // The factor has four 4-byte long words.
    // ---- First 4-byte word -----
    // First 2 bytes: 16-bit flag that indicates the type of data modification when data is being modified by a continuous process.
    // When no modification, all bits =0
    // Bit # meaning when bit is set
    // 0-reserved, 1-up sampled from lower rate, 2-downsampled from lower rate,
    // 3-Magnitude filtered, 4-Estimated magnitude, 5-estimated angle,
    // 6-Phasor magnitude adjusted for calibration, 7-phasor phase adjusted for calibration,
    // 8-phasor phase adjusted for offset (+/- 30deg, +/-120 deg, etc.)
    // 9-Psuedo-phasor value (combined from other phasors)
    // 10-14 reserved for future
    // 15 Modification applied. Type not defined here. ???

    // Third Byte: Phasor type indication (Bit 0 is LSb, Bit7 is the MSb)
    // Bits 07-04: reserved = 0
    // Bit 03: 0-voltage, 1-current
    // Bits 02-00: Phasor Component
    // 111: reserved, 110: phase C, 101: Phase B, 100: Phase A
    // 011: reserved, 010: Negative Sequence, 001: Positive Sequence, 000: Zero

    // Fourth Byte: Available for User designation

    // ---- Second and Third 4-byte words
    // Seconde 4 Byte word = Scale factor Y in 32-bit IEEE floating point.
    // Third 4 Byte word = phasor angle adjustment in radians.

    // ---- Fourthe 4 Byte word
    // Voltage class in 32-bit IEEE floating point format
    pub frscale: [u16; 8], //??? 8XFRNMR, Conversion factor for frequency channels
    // First 4 Bytes, magnitude scaling in 32bit floating point
    // Last 4 bytes, offset B in 32-bit floating point.
    pub dfdtscale: [u16; 8], //??? 8XDFDTNMR, conversion factor for ROCOF channels, Same as FRSCALE
    pub anscale: [u16; 8],   //??? 8XANNMR, conversion factor for annalog channels, same as FRSCALE
    pub digunit: [u16; 4],   //??? 4XDGNMR, Mask words for digital status words? TODO re-read
    pub pmu_lat: u32, // Latitude in Degrees, WGS84, -90 to 90, 32bit IEEE floating point, infinity for unspecified locations?
    pub pmu_lon: u32, // Longitude in Degress, WGS84, -179.99999999 to +180, 32bit IEEE floating point, unspecified=infinity
    pub pmu_elev: u32, // PMU elevation in meters, WGS84, Positive values for above mean sea level. IEEE 32 bit float, unspecified=infinity
    pub pmuflag: u16,  //
    // Bit15 1=PMU does not accept any configuration commands, 0=PMU accepts configuration commands.
    // Bit14 1=Data stream auto starts on power up., 0=Data Stream does not auto start on power up.
    // Bit13: 1=50hz, 0=60hz nominal frequency
    // Bit12: 1=Data attributes included in stream, 0=not included
    // Bit11: 1=Data available for retrieval at this PMU, 0=Not available
    // Bits10-4: Reserved- set to 0
    // Bits3-0: Data Filter used. 16 possible combinations
    // 0=P Class
    // 1=M Class
    // 2-7: Reserved
    // 8-15: User defined.
    pub window: i32, //Phasor measurement window length in microseconds, including all measurements and estimation windows in effect.
    // A value of -1 indictes the window length is not available
    pub grp_dly: i32, //Phasor measurement group delay (in microseconds) including all filters and estimation windows in effect.
    // A value of -1 indicates the group delay is not available.
    pub pmu_data_rate: i16, // Rate of data transmission for the PMU.
    // If PMU_DATA_RATE > 0, rate is # frames per second (15=15 frames per second.)
    // If PMU_DATA_RATE < 0, rate is negative of seconds per frame (-5 = 1 frame per 5 seconds)
    pub cfgcnt: u16, // Configuration change count. Value is incremented each time a change is made to PMU.
                     // 0 is factory default and initial value.
}
