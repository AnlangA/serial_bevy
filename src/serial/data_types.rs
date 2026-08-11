//! # Data Types Module
//!
//! Data encoding types for serial communication.

use std::fmt;

/// Data encoding type for serial communication.
///
/// This enum defines the supported data encoding formats for serial port data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// Binary data.
    Binary,
    /// Hexadecimal encoding.
    Hex,
    /// UTF-8 text.
    Utf8,
    /// UTF-16 text.
    Utf16,
    /// UTF-32 text.
    Utf32,
    /// GBK encoding.
    Gbk,
    /// ASCII text.
    Ascii,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binary => write!(f, "Binary"),
            Self::Hex => write!(f, "Hex"),
            Self::Utf8 => write!(f, "UTF-8"),
            Self::Utf16 => write!(f, "UTF-16"),
            Self::Utf32 => write!(f, "UTF-32"),
            Self::Gbk => write!(f, "GBK"),
            Self::Ascii => write!(f, "ASCII"),
        }
    }
}

impl DataType {
    /// Gets the English name of the data type.
    #[must_use]
    pub const fn as_str_en(&self) -> &'static str {
        match self {
            Self::Binary => "Binary",
            Self::Hex => "Hexadecimal",
            Self::Utf8 => "UTF-8",
            Self::Utf16 => "UTF-16",
            Self::Utf32 => "UTF-32",
            Self::Gbk => "GBK",
            Self::Ascii => "ASCII",
        }
    }

    /// Gets a description of the data type.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Binary => "Binary data format",
            Self::Hex => "Hexadecimal data format",
            Self::Utf8 => "UTF-8 text encoding",
            Self::Utf16 => "UTF-16 text encoding",
            Self::Utf32 => "UTF-32 text encoding",
            Self::Gbk => "GBK Chinese encoding",
            Self::Ascii => "ASCII text encoding",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_display() {
        assert_eq!(format!("{}", DataType::Hex), "Hex");
        assert_eq!(format!("{}", DataType::Utf8), "UTF-8");
        assert_eq!(format!("{}", DataType::Binary), "Binary");
        assert_eq!(format!("{}", DataType::Utf16), "UTF-16");
        assert_eq!(format!("{}", DataType::Utf32), "UTF-32");
        assert_eq!(format!("{}", DataType::Gbk), "GBK");
        assert_eq!(format!("{}", DataType::Ascii), "ASCII");
    }

    #[test]
    fn test_data_type_as_str_en() {
        assert_eq!(DataType::Hex.as_str_en(), "Hexadecimal");
        assert_eq!(DataType::Utf8.as_str_en(), "UTF-8");
        assert_eq!(DataType::Binary.as_str_en(), "Binary");
    }

    #[test]
    fn test_data_type_description() {
        assert_eq!(DataType::Binary.description(), "Binary data format");
        assert_eq!(DataType::Hex.description(), "Hexadecimal data format");
        assert_eq!(DataType::Utf8.description(), "UTF-8 text encoding");
    }
}
