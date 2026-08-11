//! # Encoding Module
//!
//! This module provides data encoding and decoding functionality for serial communication.
//! It supports various encoding formats including Hex and UTF-8.

use crate::error::SerialBevyError;
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
/// Encoded bytes, or an error when hexadecimal input contains an invalid token.
///
/// # Examples
///
/// ```
/// use serial_bevy::serial::encoding::encode_string;
/// use serial_bevy::serial::port::DataType;
///
/// let bytes = encode_string("48656C6C6F", DataType::Hex)?;
/// assert_eq!(bytes, vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
///
/// let bytes = encode_string("Hello", DataType::Utf8)?;
/// assert_eq!(bytes, vec![72, 101, 108, 108, 111]);
/// # Ok::<(), serial_bevy::error::SerialBevyError>(())
/// ```
pub fn encode_string(source_data: &str, data_type: DataType) -> crate::error::Result<Vec<u8>> {
    match data_type {
        DataType::Hex => encode_hex(source_data),
        DataType::Utf8 => Ok(source_data.as_bytes().to_vec()),
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
    }
}

/// Encodes a hex string to bytes.
///
/// Separators are whitespace, `_`, `,`, `:`, or `-`. Tokens may use a `0x`
/// prefix. An odd total digit count is padded with a leading zero.
fn encode_hex(source_data: &str) -> crate::error::Result<Vec<u8>> {
    let mut hex_str = String::with_capacity(source_data.len());
    for token in source_data.split(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '_' | ',' | ':' | '-')
    }) {
        if token.is_empty() {
            continue;
        }
        let digits = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        if digits.is_empty()
            || !digits
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(SerialBevyError::encoding(format!(
                "invalid hexadecimal token '{token}'"
            )));
        }
        hex_str.push_str(digits);
    }

    let cleaned_hex = if !hex_str.len().is_multiple_of(2) {
        format!("0{hex_str}")
    } else {
        hex_str
    };

    hex::decode(cleaned_hex).map_err(|error| SerialBevyError::encoding(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_hex_simple() {
        let result = encode_string("48656C6C6F", DataType::Hex);
        assert_eq!(result.unwrap(), vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn test_encode_hex_with_spaces() {
        let result = encode_string("48 65 6C 6C 6F", DataType::Hex);
        assert_eq!(result.unwrap(), vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn test_encode_hex_odd_length() {
        let result = encode_string("F", DataType::Hex);
        assert_eq!(result.unwrap(), vec![0x0F]);
    }

    #[test]
    fn test_encode_utf8() {
        let result = encode_string("Hello", DataType::Utf8);
        assert_eq!(result.unwrap(), vec![72, 101, 108, 108, 111]);
    }

    #[test]
    fn test_encode_hex_with_prefixes_and_separators() {
        let result = encode_string("0x48, 0X65:6c-6C_6f", DataType::Hex);

        assert_eq!(result.unwrap(), b"Hello");
    }

    #[test]
    fn test_encode_hex_rejects_invalid_token() {
        let error = encode_string("48 nope 65", DataType::Hex).unwrap_err();

        assert!(error.to_string().contains("nope"));
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
}
