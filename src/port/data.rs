use std::fs::File;

pub struct PortData {
    source_file: Vec<File>,
    parse_file: Vec<File>,
    state: State,
    data_type: Type,
}

/// serial port state
#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Open,
    Close,
    Error,
}

/// serial port data type
#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    Binary,
    Hex,
    Utf8,
    Utf16,
    Utf32,
    GBK,
    GB2312,
    ASCII,
}
