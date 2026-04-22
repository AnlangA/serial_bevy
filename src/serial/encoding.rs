//! # Encoding Module
//!
//! This module provides data encoding and decoding functionality for serial communication.
//! It supports various encoding formats including Hex and UTF-8.

use log::error;
use regex::Regex;
use std::sync::OnceLock;

use crate::serial::port::DataType;

/// Cached regex for hex sanitization.
fn hex_regex() -> &'static Regex {
    static HEX_RE: OnceLock<Regex> = OnceLock::new();
    HEX_RE.get_or_init(|| Regex::new(r"[^0-9a-fA-F]").expect("Invalid regex pattern"))
}

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
        DataType::Utf8 | DataType::Ascii | DataType::Binary => source_data.as_bytes().to_vec(),
        DataType::Utf16 => source_data
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect(),
        DataType::Utf32 => source_data
            .chars()
            .flat_map(|c| u32::from(c).to_le_bytes())
            .collect(),
        DataType::Gbk => {
            let (encoded, _, _) = encoding_rs::GBK.encode(source_data);
            encoded.into_owned()
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
        DataType::Utf8 | DataType::Ascii => {
            String::from_utf8_lossy(source_data).replace('\u{FFFD}', "❓")
        }
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
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            codepoints
                .iter()
                .map(|&cp| char::from_u32(cp).unwrap_or('\u{FFFD}'))
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
    let hex_str = hex_regex().replace_all(source_data, "");

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

    #[test]
    fn test_encode_utf16() {
        let result = encode_string("Hello", DataType::Utf16);
        // UTF-16LE: H=0x48 0x00, e=0x65 0x00, ...
        assert_eq!(result, vec![0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00]);
    }

    #[test]
    fn test_decode_utf16() {
        let result = decode_bytes(
            &[0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00],
            DataType::Utf16,
        );
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_encode_utf32() {
        let result = encode_string("AB", DataType::Utf32);
        // UTF-32LE: A=0x41 0x00 0x00 0x00, B=0x42 0x00 0x00 0x00
        assert_eq!(result, vec![0x41, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_decode_utf32() {
        let result = decode_bytes(
            &[0x41, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00],
            DataType::Utf32,
        );
        assert_eq!(result, "AB");
    }

    #[test]
    fn test_encode_gbk() {
        let result = encode_string("中文", DataType::Gbk);
        let expected = encoding_rs::GBK.encode("中文").0.into_owned();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_decode_gbk() {
        let encoded = encoding_rs::GBK.encode("中文").0.into_owned();
        let result = decode_bytes(&encoded, DataType::Gbk);
        assert_eq!(result, "中文");
    }

    #[test]
    fn test_encode_decode_ascii() {
        let encoded = encode_string("Hello", DataType::Ascii);
        assert_eq!(encoded, vec![72, 101, 108, 108, 111]);
        let decoded = decode_bytes(&encoded, DataType::Ascii);
        assert_eq!(decoded, "Hello");
    }
}
