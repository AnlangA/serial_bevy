//! # Encoding Module
//!
//! This module provides data encoding and decoding functionality for serial communication.
//! It supports various encoding formats including Hex and UTF-8.

use log::error;
use regex::Regex;

use crate::serial::port::DataType;

/// Encodes a string to bytes based on the specified data type.
///
/// # Arguments
///
/// * `source_data` - The string to encode
/// * `data_type` - The target encoding type
///
/// # Returns
///
/// A vector of bytes representing the encoded data.
///
/// # Examples
///
/// ```
/// use serial_bevy::serial::encoding::encode_string;
/// use serial_bevy::serial::port::DataType;
///
/// let bytes = encode_string("48656C6C6F", DataType::Hex);
/// assert_eq!(bytes, vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
///
/// let bytes = encode_string("Hello", DataType::Utf8);
/// assert_eq!(bytes, vec![72, 101, 108, 108, 111]);
/// ```
#[must_use]
pub fn encode_string(source_data: &str, data_type: DataType) -> Vec<u8> {
    match data_type {
        DataType::Hex => encode_hex(source_data),
        DataType::Utf8 => source_data.as_bytes().to_vec(),
        DataType::Ascii => source_data.as_bytes().to_vec(),
        DataType::Binary => source_data.as_bytes().to_vec(),
        DataType::Utf16 | DataType::Utf32 | DataType::Gbk => {
            let encoded = encoding_rs::GBK.encode(source_data);
            encoded.0.into_owned()
        }
    }
}

/// Decodes bytes to a string based on the specified data type.
///
/// # Arguments
///
/// * `source_data` - The bytes to decode
/// * `data_type` - The source encoding type
///
/// # Returns
///
/// A string representing the decoded data.
///
/// # Examples
///
/// ```
/// use serial_bevy::serial::encoding::decode_bytes;
/// use serial_bevy::serial::port::DataType;
///
/// let text = decode_bytes(&[0x48, 0x65, 0x6C, 0x6C, 0x6F], DataType::Hex);
/// assert_eq!(text, "48656c6c6f");
///
/// let text = decode_bytes(&[72, 101, 108, 108, 111], DataType::Utf8);
/// assert_eq!(text, "Hello");
/// ```
#[must_use]
pub fn decode_bytes(source_data: &[u8], data_type: DataType) -> String {
    match data_type {
        DataType::Hex => hex::encode(source_data),
        DataType::Utf8 => String::from_utf8_lossy(source_data).replace('�', "❓"),
        DataType::Ascii => String::from_utf8_lossy(source_data).replace('�', "❓"),
        DataType::Binary => source_data
            .iter()
            .map(|b| format!("{:08b}", b))
            .collect::<Vec<_>>()
            .join(" "),
        DataType::Utf16 => {
            let (decoded, _, _) = encoding_rs::UTF_16LE.decode(source_data);
            decoded.into_owned()
        }
        DataType::Utf32 => {
            let codepoints: Vec<u32> = source_data
                .chunks(4)
                .filter_map(|chunk| {
                    if chunk.len() == 4 {
                        Some(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    } else {
                        None
                    }
                })
                .collect();
            codepoints
                .iter()
                .map(|&cp| char::from_u32(cp).unwrap_or('�'))
                .collect()
        }
        DataType::Gbk => {
            let (decoded, _, _) = encoding_rs::GBK.decode(source_data);
            decoded.into_owned()
        }
    }
}

/// Encodes a hex string to bytes.
///
/// This function removes all non-hex characters and pads with a leading zero
/// if the string has an odd length.
fn encode_hex(source_data: &str) -> Vec<u8> {
    let re = Regex::new(r"[^0-9a-fA-F]").expect("Invalid regex pattern");
    let hex_str = re.replace_all(source_data, "");

    let cleaned_hex = if !hex_str.len().is_multiple_of(2) {
        format!("0{hex_str}")
    } else {
        hex_str.to_string()
    };

    let bytes_result: Result<Vec<u8>, _> = (0..cleaned_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&cleaned_hex[i..i + 2], 16))
        .collect();

    match bytes_result {
        Ok(bytes) => bytes,
        Err(err) => {
            error!("Hex encoding error: {err}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_hex_simple() {
        let result = encode_string("48656C6C6F", DataType::Hex);
        assert_eq!(result, vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn test_encode_hex_with_spaces() {
        let result = encode_string("48 65 6C 6C 6F", DataType::Hex);
        assert_eq!(result, vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn test_encode_hex_odd_length() {
        let result = encode_string("F", DataType::Hex);
        assert_eq!(result, vec![0x0F]);
    }

    #[test]
    fn test_encode_utf8() {
        let result = encode_string("Hello", DataType::Utf8);
        assert_eq!(result, vec![72, 101, 108, 108, 111]);
    }

    #[test]
    fn test_decode_hex() {
        let result = decode_bytes(&[0x48, 0x65, 0x6C, 0x6C, 0x6F], DataType::Hex);
        assert_eq!(result, "48656c6c6f");
    }

    #[test]
    fn test_decode_utf8() {
        let result = decode_bytes(&[72, 101, 108, 108, 111], DataType::Utf8);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_decode_utf8_invalid() {
        let result = decode_bytes(&[0xFF, 0xFE], DataType::Utf8);
        assert!(result.contains('❓'));
    }

    #[test]
    fn test_encode_binary() {
        let result = encode_string("test", DataType::Binary);
        assert_eq!(result, b"test");
    }

    #[test]
    fn test_decode_binary() {
        let result = decode_bytes(&[1, 2, 3], DataType::Binary);
        assert!(!result.is_empty());
    }
}
