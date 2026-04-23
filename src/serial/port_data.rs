//! # Port Data Module
//!
//! This module provides data management for serial port communication,
//! including file logging, display buffering, and UTF-8 processing.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{BufWriter, Read, Write};

use log::{error, warn};

use super::data_types::DataType;
use super::port::CacheData;
use super::state::{DataSource, PortState};

/// File data storage.
struct FileData {
    /// List of file paths.
    file: Vec<String>,
}

/// Port data management for files and communication.
pub struct PortData {
    /// Source file paths for logging.
    source_file: FileData,
    /// Data pending to be sent.
    send_data: Vec<String>,
    /// Command cache and history.
    cache_data: CacheData,
    /// Current port state.
    state: PortState,
    /// Data encoding type.
    data_type: DataType,
    /// Whether to include line feeds in sent data.
    line_feed: bool,
    /// Buffer for incomplete UTF-8 sequences.
    utf8_buffer: Vec<u8>,
    /// Console mode flag - provides better terminal experience for Linux serial consoles.
    /// When enabled: no timestamps, local echo, line-buffered sending.
    console_mode: bool,
    /// Show timestamp in log file and display.
    /// When false (default): raw data format without timestamps.
    /// When true: adds [timestamp source] prefix to each line.
    show_timestamp: bool,
    /// In-memory display buffer to avoid reading disk every frame.
    display_buffer: VecDeque<String>,
    /// Accumulated display text cache for efficient reading.
    /// Updated in sync with `display_buffer` to avoid rebuilding every frame.
    display_text: String,
    /// Persistent file writer for logging.
    file_writer: Option<BufWriter<std::fs::File>>,
}

impl Default for PortData {
    fn default() -> Self {
        Self::new()
    }
}

impl PortData {
    /// Creates a new `PortData` instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            source_file: FileData { file: Vec::new() },
            send_data: Vec::new(),
            cache_data: CacheData::new(),
            state: PortState::Close,
            data_type: DataType::Utf8,
            line_feed: false,
            utf8_buffer: Vec::new(),
            console_mode: false,
            show_timestamp: false,
            display_buffer: VecDeque::new(),
            display_text: String::new(),
            file_writer: None,
        }
    }

    /// Adds a source file for logging under the relative `logs/` directory and returns the new file count.
    ///
    /// Sanitization rules:
    /// - Leading `/` or `\` is stripped (prevents absolute paths).
    /// - Inner `/` or `\` are replaced with `_`.
    /// - `..` components are removed to prevent directory traversal attacks.
    ///
    /// The final stored path is always `logs/<sanitized_name>`.
    /// On failure to create the file, an error is logged but the path is still recorded.
    pub fn add_source_file(&mut self, name: String) -> usize {
        // Ensure logs directory exists (best-effort; ignore errors here).
        let _ = std::fs::create_dir_all("logs");

        // Sanitize user-provided file name (e.g. "/dev/ttyUSB0_20250101_010101.txt").
        // Strip leading slashes, replace inner slashes/backslashes with underscores,
        // and remove `..` components to prevent path traversal attacks.
        let sanitized = name
            .trim_start_matches('/')
            .trim_start_matches('\\')
            .replace(['/', '\\'], "_")
            .replace("..", "");

        let path = format!("logs/{sanitized}");

        match OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
        {
            Ok(file) => {
                self.file_writer = Some(BufWriter::new(file));
            }
            Err(e) => {
                error!("Failed to create source file {path}: {e}");
                self.file_writer = None;
            }
        }

        self.source_file.file.push(path);
        self.source_file.file.len()
    }

    /// Gets the number of source files.
    #[must_use]
    pub const fn source_file_index(&self) -> usize {
        self.source_file.file.len()
    }

    /// Writes data to the last source file and memory display buffer.
    /// Format depends on show_timestamp setting:
    /// - If show_timestamp is true: writes with [timestamp source] prefix
    /// - If show_timestamp is false: writes raw data without prefix
    ///
    /// This also maintains a cached `display_text` string for efficient reads.
    /// When `display_buffer` exceeds 5000 entries, the oldest entries are trimmed
    /// from both the buffer and the cached text.
    pub fn write_source_file(&mut self, data: &[u8], source: DataSource) {
        let line = if self.show_timestamp {
            let time = chrono::Local::now()
                .format("%Y%m%d %H:%M:%S.%3f")
                .to_string();
            format!("\n[{time} {source}]{}", String::from_utf8_lossy(data))
        } else {
            String::from_utf8_lossy(data).into_owned()
        };

        // Write to persistent file writer with proper error logging
        if let Some(writer) = &mut self.file_writer {
            if let Err(e) = writer.write_all(line.as_bytes()) {
                warn!("Failed to write to source file: {e}");
            }
            if let Err(e) = writer.flush() {
                warn!("Failed to flush source file writer: {e}");
            }
        }

        // Push to memory display buffer and update cached text
        self.display_buffer.push_back(line.clone());
        self.display_text.push_str(&line);

        // Trim buffer if it exceeds the maximum size
        while self.display_buffer.len() > 5000 {
            if let Some(removed) = self.display_buffer.pop_front() {
                // Remove the same content from the front of the cached text
                let remove_len = removed.len();
                if remove_len <= self.display_text.len() {
                    self.display_text.drain(..remove_len);
                }
            }
        }
    }

    /// Reads the current display data from the in-memory cache.
    ///
    /// This uses the pre-built `display_text` cache rather than concatenating
    /// the buffer on every call, providing O(1) access to accumulated data.
    #[must_use]
    pub fn read_current_source_file_bytes(&self) -> Vec<u8> {
        self.display_text.as_bytes().to_vec()
    }

    /// Clears the in-memory display buffer and cached text for the current log view.
    pub fn clear_display_buffer(&mut self) {
        self.display_buffer.clear();
        self.display_text.clear();
    }

    /// Flushes the persistent file writer.
    pub fn flush_file_writer(&mut self) {
        if let Some(writer) = &mut self.file_writer
            && let Err(e) = writer.flush()
        {
            warn!("Failed to flush file writer: {e}");
        }
    }

    /// Reads a specific source file by index.
    #[must_use]
    pub fn read_source_file(&self, index: usize) -> String {
        self.source_file
            .file
            .get(index)
            .and_then(|path| {
                OpenOptions::new()
                    .read(true)
                    .open(path)
                    .ok()
                    .map(|mut file| {
                        let mut data = String::new();
                        let _ = file.read_to_string(&mut data);
                        data
                    })
            })
            .unwrap_or_default()
    }

    /// Gets a source file name by index.
    #[must_use]
    pub fn get_source_file_name(&self, index: usize) -> &str {
        self.source_file
            .file
            .get(index)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Queues data to be sent.
    pub fn send_data(&mut self, data: String) {
        self.send_data.push(data);
    }

    /// Gets and clears the send data queue.
    pub fn get_send_data(&mut self) -> Vec<String> {
        std::mem::take(&mut self.send_data)
    }

    /// Clears the send data queue.
    pub fn clear_send_data(&mut self) {
        self.send_data.clear();
    }

    /// Sets the data encoding type.
    pub const fn set_data_type(&mut self, data_type: DataType) {
        self.data_type = data_type;
    }

    /// Gets a mutable reference to the cache data.
    pub const fn get_cache_data(&mut self) -> &mut CacheData {
        &mut self.cache_data
    }

    /// Gets a mutable reference to the port state.
    pub const fn state(&mut self) -> &mut PortState {
        &mut self.state
    }

    /// Gets a reference to the port state (read-only).
    #[must_use]
    pub const fn state_ref(&self) -> &PortState {
        &self.state
    }

    /// Gets a mutable reference to the data type.
    pub const fn data_type(&mut self) -> &mut DataType {
        &mut self.data_type
    }

    /// Gets a mutable reference to the line feed setting.
    pub const fn line_feed(&mut self) -> &mut bool {
        &mut self.line_feed
    }

    /// Gets a mutable reference to the console mode setting.
    pub const fn console_mode(&mut self) -> &mut bool {
        &mut self.console_mode
    }

    /// Returns true if console mode is enabled.
    #[must_use]
    pub const fn is_console_mode(&self) -> bool {
        self.console_mode
    }

    /// Gets a mutable reference to the show timestamp setting.
    pub const fn show_timestamp(&mut self) -> &mut bool {
        &mut self.show_timestamp
    }

    /// Returns true if timestamps should be shown.
    #[must_use]
    pub const fn is_show_timestamp(&self) -> bool {
        self.show_timestamp
    }

    /// Processes raw bytes with UTF-8 buffer handling.
    /// Also normalizes line endings: converts \r\n to \n and removes standalone \r
    pub fn process_raw_bytes(&mut self, data: &[u8]) -> Vec<u8> {
        // Add new data to buffer
        self.utf8_buffer.extend_from_slice(data);

        // Try to decode as much as possible
        let (valid_str, incomplete_len) = self.extract_valid_utf8();

        // Remove processed bytes from buffer
        if incomplete_len > 0 {
            self.utf8_buffer
                .drain(..(self.utf8_buffer.len() - incomplete_len));
        } else {
            self.utf8_buffer.clear();
        }

        // Normalize line endings: \r\n -> \n, standalone \r -> \n
        let normalized = valid_str.replace("\r\n", "\n").replace('\r', "\n");

        normalized.into_bytes()
    }

    /// Extracts valid UTF-8 from buffer, returns (valid_string, incomplete_bytes_count)
    fn extract_valid_utf8(&self) -> (String, usize) {
        if self.utf8_buffer.is_empty() {
            return (String::new(), 0);
        }

        // Try to decode the entire buffer
        match std::str::from_utf8(&self.utf8_buffer) {
            Ok(valid_str) => {
                // All bytes are valid UTF-8
                (valid_str.to_string(), 0)
            }
            Err(e) => {
                let valid_len = e.valid_up_to();
                if valid_len > 0 {
                    // We have some valid UTF-8 at the beginning
                    let valid_str =
                        std::str::from_utf8(&self.utf8_buffer[..valid_len]).unwrap_or("�");
                    (valid_str.to_string(), self.utf8_buffer.len() - valid_len)
                } else {
                    // No valid UTF-8 at start, check if we have incomplete UTF-8 at end
                    let incomplete_len = self.count_incomplete_utf8_suffix();
                    if incomplete_len > 0 && incomplete_len < 4 {
                        // Likely incomplete UTF-8 sequence, keep it for next time
                        let valid_len = self.utf8_buffer.len() - incomplete_len;
                        if valid_len > 0 {
                            let valid_str =
                                std::str::from_utf8(&self.utf8_buffer[..valid_len]).unwrap_or("�");
                            (valid_str.to_string(), incomplete_len)
                        } else {
                            // All bytes are incomplete, keep them all
                            (String::new(), incomplete_len)
                        }
                    } else {
                        // Invalid UTF-8, replace with replacement char
                        ("�".to_string(), 0)
                    }
                }
            }
        }
    }

    /// Counts incomplete UTF-8 sequence at the end of buffer
    fn count_incomplete_utf8_suffix(&self) -> usize {
        if self.utf8_buffer.is_empty() {
            return 0;
        }

        // Check last 1-3 bytes for incomplete UTF-8 sequence
        let len = self.utf8_buffer.len();
        let check_len = std::cmp::min(3, len);

        for i in 1..=check_len {
            let start = len - i;
            let slice = &self.utf8_buffer[start..];

            // Check if this could be the start of a UTF-8 sequence
            if slice[0] >= 0x80 {
                // Check if this is a continuation byte or start of multi-byte sequence
                // Check if it is a valid UTF-8 start byte
                if (slice[0] & 0xE0) == 0xC0 && (1..=2).contains(&i) {
                    // 2-byte sequence
                    return if i == 1 { 1 } else { 0 };
                } else if (slice[0] & 0xF0) == 0xE0 && (1..=3).contains(&i) {
                    // 3-byte sequence
                    return if i <= 2 { i } else { 0 };
                } else if (slice[0] & 0xF8) == 0xF0 && (1..=4).contains(&i) {
                    // 4-byte sequence
                    return if i <= 3 { i } else { 0 };
                } else if (slice[0] & 0xC0) == 0x80 {
                    // Continuation byte
                    return i;
                }
            }
        }

        0
    }

    /// Clears the UTF-8 buffer.
    pub fn clear_utf8_buffer(&mut self) {
        self.utf8_buffer.clear();
    }
}
