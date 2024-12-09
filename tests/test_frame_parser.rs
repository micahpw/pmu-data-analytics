#![allow(unused)]
use std::cmp::min;
use std::fs;
use std::path::Path;

fn read_hex_file(file_name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = Path::new("tests/test_data").join(file_name);
    let content = fs::read_to_string(path)?;
    let hex_string: String = content.chars().filter(|c| !c.is_whitespace()).collect();

    hex_string
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let hex_byte = std::str::from_utf8(chunk).unwrap();
            u8::from_str_radix(hex_byte, 16).map_err(|e| e.into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pmu::frame_parser::{parse_config_frame_1and2, parse_data_frames};
    use pmu::frames::{
        calculate_crc, ConfigurationFrame1and2_2011, DataFrame2011, PMUConfigurationFrame2011,
        PMUFrameType, PMUValues, PrefixFrame2011,
    };

    #[test]
    fn test_calculate_crc_standard_values() {
        // Test values from Table B.1 of IEEE C37.118.2-2011 standard
        let test_cases = [
            (vec![0x41, 0x42, 0x43, 0x44], 0xBFFA),
            (vec![0x31, 0x32, 0x33, 0x34, 0x35, 0x36], 0x2EF4),
            (vec![0x61, 0x62, 0x63], 0x514A),
        ];

        for (input, expected_crc) in test_cases.iter() {
            let calculated_crc = calculate_crc(input);
            assert_eq!(
                calculated_crc, *expected_crc,
                "CRC mismatch for input: {:?}",
                input
            );
        }
    }
    #[test]
    fn test_crc_validation() {
        let files = ["cmd_message.bin", "config_message.bin", "data_message.bin"];

        for file in files.iter() {
            let buffer = super::read_hex_file(file).unwrap();
            let frame_size = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
            let calculated_crc = calculate_crc(&buffer[..frame_size - 2]);
            let frame_crc = u16::from_be_bytes([buffer[frame_size - 2], buffer[frame_size - 1]]);

            assert_eq!(calculated_crc, frame_crc, "CRC mismatch for file: {}", file);
        }
    }

    #[test]
    fn test_command_frame_to_hex() {
        use pmu::frames::CommandFrame2011;

        // Create a CommandFrame2011 struct with the expected values
        let command_frame = CommandFrame2011 {
            prefix: PrefixFrame2011 {
                sync: 0xAA41,
                framesize: 18,
                idcode: 7734,
                soc: 1149591600,
                fracsec: 252428240, //byteorder=bigendian
            },
            command: 2,
            extframe: None,
            chk: 0, //to be filled by to_hex method.
        };

        // Convert the struct to a Vec<u8>
        let frame_bytes = command_frame.to_hex();

        // Read the hex file
        let file_bytes = super::read_hex_file("cmd_message.bin").unwrap();

        // Compare the generated bytes with the file contents
        assert_eq!(
            frame_bytes, file_bytes,
            "Generated command frame does not match the file contents"
        );
    }

    // Tests the parse_config_frame1and2_2011 function
    // Uses test data from the IEEE C37.118.2 2011 standard.
    // Tests that certain values are parsed correctly.
    #[test]
    fn test_parse_config_frame() {
        let buffer = super::read_hex_file("config_message.bin").unwrap();
        let result = parse_config_frame_1and2(&buffer);

        assert!(result.is_ok(), "Failed to parse configuration frame");

        let config_frame = result.unwrap();

        // Add assertions to verify the parsed data
        println!("Config frame prefix: {:?}", config_frame.prefix);
        assert_eq!(config_frame.prefix.framesize, 454);
        assert_eq!(config_frame.prefix.idcode, 7734);
        assert_eq!(config_frame.time_base, 1000000);
        assert_eq!(config_frame.num_pmu, 1);
        assert_eq!(config_frame.data_rate, 30);

        // Verify PMU configuration
        let pmu_config = &config_frame.pmu_configs[0];
        //assert_eq!(pmu_config.stn, *b"Station A        ");
        assert_eq!(pmu_config.idcode, 7734);
        assert_eq!(pmu_config.format, 4);
        assert_eq!(pmu_config.phnmr, 4);
        assert_eq!(pmu_config.annmr, 3);
        assert_eq!(pmu_config.dgnmr, 1);

        // TODO Add more assertions as needed to verify other fields
        // Verify CRC
        let calculated_crc = calculate_crc(&buffer[..buffer.len() - 2]);
        assert_eq!(
            calculated_crc, config_frame.chk,
            "CRC mismatch in configuration frame"
        );
    }
    #[test]
    fn test_parse_config_frame_multi() {
        let buffer = super::read_hex_file("config_message_multi.bin").unwrap();
        let result = parse_config_frame_1and2(&buffer);

        assert!(result.is_ok(), "Failed to parse configuration frame");

        let config_frame = result.unwrap();

        // Add assertions to verify the parsed data
        println!("Config frame prefix: {:?}", config_frame.prefix);
        assert_eq!(config_frame.prefix.framesize, 884);
        assert_eq!(config_frame.prefix.idcode, 7734);
        assert_eq!(config_frame.time_base, 1000000);
        assert_eq!(config_frame.num_pmu, 2);
        assert_eq!(config_frame.data_rate, 30);

        // Verify PMU configuration
        let pmu_config = &config_frame.pmu_configs[0];
        //assert_eq!(pmu_config.stn, *b"Station A        ");
        assert_eq!(pmu_config.idcode, 7734);
        assert_eq!(pmu_config.format, 4);
        assert_eq!(pmu_config.phnmr, 4);
        assert_eq!(pmu_config.annmr, 3);
        assert_eq!(pmu_config.dgnmr, 1);

        // TODO Add more assertions as needed to verify other fields
        // Verify CRC
        let calculated_crc = calculate_crc(&buffer[..buffer.len() - 2]);
        assert_eq!(
            calculated_crc, config_frame.chk,
            "CRC mismatch in configuration frame"
        );
    }

    #[test]
    fn test_pmu_config_serialization() {
        use super::*;

        // Create a sample PMU configuration
        let config_buffer = super::read_hex_file("config_message.bin").unwrap();
        let config_frame = parse_config_frame_1and2(&config_buffer).unwrap();
        let pmu_config = &config_frame.pmu_configs[0];

        // Serialize to JSON
        let json = serde_json::to_string_pretty(pmu_config).unwrap();
        println!("Serialized PMU Config:\n{}", json);

        // Parse the JSON back into a Value for verification
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Verify specific fields
        assert_eq!(parsed["idcode"], 7734);
        assert_eq!(parsed["phnmr"], 4);
        assert_eq!(parsed["annmr"], 3);
        assert_eq!(parsed["dgnmr"], 1);

        // Verify station name is properly decoded
        assert_eq!(parsed["stn"], "Station A");

        // Verify channel names are present and correct
        let channels = parsed["channels"].as_array().unwrap();
        assert!(!channels.is_empty());

        // Verify format flags
        let format_flags = &parsed["format_flags"];
        assert_eq!(format_flags["freq_dfreq_float"], false);
        assert_eq!(format_flags["analog_float"], true);
        assert_eq!(format_flags["phasor_float"], false);
        assert_eq!(format_flags["phasor_polar"], false);

        // Verify computed properties
        assert_eq!(parsed["is_polar"], false);
    }
    #[test]
    fn test_calc_data_frame_size() {
        // Parse the configuration frame
        let config_buffer = super::read_hex_file("config_message.bin").unwrap();
        let config_frame = parse_config_frame_1and2(&config_buffer).unwrap();

        // Calculate expected frame size
        let calculated_size = config_frame.calc_data_frame_size();

        // Get actual frame size from data_message.bin
        let data_buffer = super::read_hex_file("data_message.bin").unwrap();
        let actual_size = data_buffer.len();

        // Get framesize from prefix
        let prefix_size = u16::from_be_bytes([data_buffer[2], data_buffer[3]]);

        // All sizes should match
        assert_eq!(calculated_size, actual_size);
        assert_eq!(calculated_size, prefix_size as usize);
        assert_eq!(actual_size as u16, prefix_size);
    }

    #[test]
    fn test_parse_data_frame() {
        // First, parse the configuration frame
        let config_buffer = super::read_hex_file("config_message.bin").unwrap();
        let config_result = parse_config_frame_1and2(&config_buffer);
        assert!(config_result.is_ok(), "Failed to parse configuration frame");
        let config_frame = config_result.unwrap();

        let pmu_config = &config_frame.pmu_configs[0];
        println!("phnmr: {}", pmu_config.phnmr);
        println!("annmr: {}", pmu_config.annmr);
        println!("phasor_usize: {}", pmu_config.phasor_size());
        println!("analog_usize: {}", pmu_config.analog_size());
        println!("freq_dfreq_usize: {}", pmu_config.freq_dfreq_size());

        // Now, parse the data frame
        let data_buffer = super::read_hex_file("data_message.bin").unwrap();
        let data_result = parse_data_frames(&data_buffer, &config_frame);
        assert!(data_result.is_ok(), "Failed to parse data frame");
        let data_frame = data_result.unwrap();

        // Add assertions to verify the parsed data
        // All test data is based on Table D.1 of IEEE C37.118.2 - 2011
        assert_eq!(data_frame.prefix.framesize, 52);
        assert_eq!(data_frame.prefix.idcode, 7734);
        assert_eq!(data_frame.prefix.soc, 1149580800);
        assert_eq!(data_frame.prefix.fracsec, 16817);

        // Verify PMU data
        assert_eq!(data_frame.data.len(), 1);
        //let pmu_data = &data_frame.data[0];

        //assert_eq!(pmu_data.stat, 0x0000);
        // Verify PMU data
        let pmu_data = match &data_frame.data[0] {
            PMUFrameType::Fixed(data) => data,
            _ => panic!("Expected PMUDataFrameFloating"),
        };

        assert_eq!(pmu_data.stat, 0x0000);
        // Verify phasors, frequency, dfreq, analog, and digital values
        // Note: These assertions might need adjustment based on your exact parsing logic
        assert_eq!(pmu_data.phasors.len(), 16); // Size in Bytes
        assert_eq!(pmu_data.freq, 2500);
        assert_eq!(pmu_data.dfreq, 0);
        assert_eq!(pmu_data.analog.len(), 12); // Size in Bytes
        assert_eq!(pmu_data.digital.len(), 2); // Size in Bytes

        let phasor_values = match &data_frame.data[0] {
            PMUFrameType::Fixed(data) => data.parse_phasors(pmu_config),
            PMUFrameType::Floating(data) => data.parse_phasors(pmu_config),
        };

        let is_polar = pmu_config.is_phasor_polar();
        assert_eq!(is_polar, false);

        // Test Phasor values
        assert_eq!(phasor_values[0], PMUValues::Fixed(vec![14635, 0]));
        assert_eq!(phasor_values[1], PMUValues::Fixed(vec![-7318, -12676]));
        assert_eq!(phasor_values[2], PMUValues::Fixed(vec![-7318, 12675]));
        assert_eq!(phasor_values[3], PMUValues::Fixed(vec![1092, 0]));

        // Test Analog Values
        let analog_values = match &data_frame.data[0] {
            PMUFrameType::Fixed(data) => data.parse_analogs(pmu_config),
            PMUFrameType::Floating(data) => data.parse_analogs(pmu_config),
        };

        assert_eq!(
            analog_values,
            PMUValues::Float(vec![100.0, 1000.0, 10000.0])
        );
        // Test Digital Values
        let digital_values = match &data_frame.data[0] {
            PMUFrameType::Fixed(data) => data.parse_digitals(),
            PMUFrameType::Floating(data) => data.parse_digitals(),
        };

        println!("Digital Values: {:016b}", digital_values[0]); // Display as 16-bit binary

        assert_eq!(digital_values[0], 0b0011110000010010); // Test all alternating high/low bits
                                                           // Verify CRC
        let calculated_crc = calculate_crc(&data_buffer[..data_buffer.len() - 2]);
        assert_eq!(calculated_crc, data_frame.chk, "CRC mismatch in data frame");
    }
    #[test]
    fn test_channel_name_extraction() {
        let config_buffer = super::read_hex_file("config_message.bin").unwrap();
        let config_frame = parse_config_frame_1and2(&config_buffer).unwrap();
        let pmu_config = &config_frame.pmu_configs[0];

        // Test phasor column names
        let phasor_columns = pmu_config.get_phasor_columns();
        assert_eq!(phasor_columns.len(), 4);
        assert_eq!(phasor_columns[0], "Station A_7734_VA");
        assert_eq!(phasor_columns[1], "Station A_7734_VB");
        assert_eq!(phasor_columns[2], "Station A_7734_VC");
        assert_eq!(phasor_columns[3], "Station A_7734_I1");

        // Test analog column names
        let analog_columns = pmu_config.get_analog_columns();
        assert_eq!(analog_columns.len(), 3);
        assert_eq!(analog_columns[0], "Station A_7734_ANALOG1");
        assert_eq!(analog_columns[1], "Station A_7734_ANALOG2");
        assert_eq!(analog_columns[2], "Station A_7734_ANALOG3");

        // Test digital column names
        let digital_columns = pmu_config.get_digital_columns();
        assert_eq!(digital_columns.len(), 1);
        assert_eq!(digital_columns[0], "Station A_7734_BREAKER 1 STATUS");
    }

    #[test]
    fn test_arrow_frame_creation() {
        use arrow::array::{
            Array, ArrayRef, Float32Array, Float64Array, Int16Array, Int32Array, StringArray,
            TimestampMicrosecondArray, UInt16Array,
        };
        use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
        use arrow::record_batch::RecordBatch;
        use pmu::arrow_utils::{build_arrow_schema, extract_channel_values};
        use std::cmp::min;
        use std::sync::Arc;

        // First parse the configuration frame
        let config_buffer = super::read_hex_file("config_message.bin").unwrap();
        let config_frame = parse_config_frame_1and2(&config_buffer).unwrap();

        // Get the channel map from config
        let channel_map = config_frame.get_channel_map();
        println!("\nChannel Map:");
        for (name, info) in &channel_map {
            println!("Channel: {}", name);
            println!("  Offset: {}", info.offset);
            println!("  Size: {}", info.size);
            println!("  Type: {:?}", info.data_type);
        }
        // Create a 30kB buffer and fill it with repeated data frames
        let data_frame = super::read_hex_file("data_message.bin").unwrap();
        let frame_size = data_frame.len();
        println!("\nFrame size: {}", frame_size);

        let mut buffer = vec![0u8; 30 * 1024];

        // Copy the data frame multiple times into the buffer
        let num_frames = buffer.len() / frame_size;
        for i in 0..num_frames {
            let start = i * frame_size;
            if start + frame_size <= buffer.len() {
                buffer[start..start + frame_size].copy_from_slice(&data_frame);
            }
        }

        // Build Arrow schema
        let schema = build_arrow_schema(&channel_map);
        println!("Arrow Schema: {:#?}", schema);

        // Create arrays for each channel
        let mut arrays: Vec<ArrayRef> = Vec::new();

        // Add timestamp array - explicitly convert to ArrayRef
        let mut timestamps = Vec::new();
        for i in 0..num_frames {
            let frame_start = i * frame_size;
            if frame_start + frame_size <= buffer.len() {
                let soc = u32::from_be_bytes([
                    buffer[frame_start + 6],
                    buffer[frame_start + 7],
                    buffer[frame_start + 8],
                    buffer[frame_start + 9],
                ]);
                let fracsec = u32::from_be_bytes([
                    buffer[frame_start + 10],
                    buffer[frame_start + 11],
                    buffer[frame_start + 12],
                    buffer[frame_start + 13],
                ]);
                timestamps.push((soc as i64) * 1_000_000 + (fracsec as i64));
            }
        }
        let timestamp_array: ArrayRef = Arc::new(TimestampMicrosecondArray::from(timestamps));
        arrays.push(timestamp_array);

        // Extract values for each channel
        for (name, info) in &channel_map {
            println!("\nExtracting values for channel: {}", name);
            let channel_arrays = extract_channel_values(&buffer, frame_size, info);

            // Print first few values for debugging
            println!("First few values:");
            for (i, arr) in channel_arrays.iter().enumerate() {
                print!("  Array {}: ", i);
                for j in 0..min(5, arr.len()) {
                    match arr.data_type() {
                        DataType::Int16 => print!(
                            "{:?} ",
                            arr.as_any().downcast_ref::<Int16Array>().unwrap().value(j)
                        ),
                        DataType::Float32 => print!(
                            "{:?} ",
                            arr.as_any()
                                .downcast_ref::<Float32Array>()
                                .unwrap()
                                .value(j)
                        ),
                        DataType::UInt16 => print!(
                            "{:?} ",
                            arr.as_any().downcast_ref::<UInt16Array>().unwrap().value(j)
                        ),
                        _ => print!("Unsupported type "),
                    }
                }
                println!();
            }
            arrays.extend(channel_arrays);
        }

        // Create RecordBatch
        let record_batch = RecordBatch::try_new(Arc::new(schema.clone()), arrays).unwrap();
        // Print the first few rows for verification
        println!("\nFirst few rows of RecordBatch:");
        for i in 0..min(5, record_batch.num_rows()) {
            print!("Row {}: ", i);
            for j in 0..record_batch.num_columns() {
                let col = record_batch.column(j);
                match col.data_type() {
                    DataType::Int16 => print!(
                        "{:?} ",
                        col.as_any().downcast_ref::<Int16Array>().unwrap().value(i)
                    ),
                    DataType::Float32 => print!(
                        "{:?} ",
                        col.as_any()
                            .downcast_ref::<Float32Array>()
                            .unwrap()
                            .value(i)
                    ),
                    DataType::UInt16 => print!(
                        "{:?} ",
                        col.as_any().downcast_ref::<UInt16Array>().unwrap().value(i)
                    ),
                    DataType::Timestamp(_, _) => print!(
                        "{:?} ",
                        col.as_any()
                            .downcast_ref::<TimestampMicrosecondArray>()
                            .unwrap()
                            .value(i)
                    ),
                    _ => print!("Unsupported type "),
                }
            }
            println!();
        }
        // Verify the record batch
        assert_eq!(record_batch.num_rows(), num_frames);
        assert_eq!(record_batch.num_columns(), schema.fields().len());

        // Test some specific values from the first row
        let pmu_config = &config_frame.pmu_configs[0];

        // Test specific values from the first row using column names
        if let Some(freq_col) = record_batch
            .column_by_name("Station A_7734_FREQ")
            .and_then(|col| col.as_any().downcast_ref::<Int16Array>())
        {
            assert_eq!(freq_col.value(0), 2500, "Frequency value mismatch");
        } else {
            panic!("Failed to get frequency column");
        }

        // Test first phasor magnitude and angle
        if let Some(mag_col) = record_batch
            .column_by_name("Station A_7734_VA_X")
            .and_then(|col| col.as_any().downcast_ref::<Int16Array>())
        {
            assert_eq!(mag_col.value(0), 14635, "Phasor X value mismatch");
        } else {
            panic!("Failed to get phasor X column");
        }

        if let Some(ang_col) = record_batch
            .column_by_name("Station A_7734_VA_Y")
            .and_then(|col| col.as_any().downcast_ref::<Int16Array>())
        {
            assert_eq!(ang_col.value(0), 0, "Phasor Y mismatch");
        } else {
            panic!("Failed to get phasor Y column");
        }

        // Test first analog value
        if let Some(analog_col) = record_batch
            .column_by_name("Station A_7734_ANALOG1")
            .and_then(|col| col.as_any().downcast_ref::<Float32Array>())
        {
            assert_eq!(analog_col.value(0), 100.0, "Analog value mismatch");
        } else {
            panic!("Failed to get analog column");
        }

        // Test digital value
        if let Some(digital_col) = record_batch
            .column_by_name("Station A_7734_BREAKER 1 STATUS")
            .and_then(|col| col.as_any().downcast_ref::<UInt16Array>())
        {
            assert_eq!(
                digital_col.value(0),
                0b0011110000010010,
                "Digital value mismatch"
            );
        } else {
            panic!("Failed to get digital column");
        } // Print column names and first few values for debugging
        println!("\nColumn values:");
        for i in 0..record_batch.num_columns() {
            println!(
                "{}: {:?}",
                schema.field(i).name(),
                record_batch.column(i).slice(0, 5)
            );
        }
    }
}
