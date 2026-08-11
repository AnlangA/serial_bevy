# Changelog

All notable changes to Serial Bevy are documented in this file.

## [0.2.0] - Unreleased

### Added

- Explicit opening state and visible, actionable serial/logging errors.
- Bounded command history, live log, command editor, and pending-send queue.
- Streaming UTF-8 decoding and strict hexadecimal-input validation.
- Portable font, preference, and session-log path fallbacks.
- Pseudoterminal serial integration tests, headless UI tests, dependency auditing,
  exact-MSRV checks, and Windows/macOS CI checks.

### Changed

- Updated to Bevy 0.19, bevy_egui 0.41, Tokio 1.53, and Rust 1.95.
- Replaced detached serial read/write tasks with one cancellable session task.
- Replaced unbounded serial traffic broadcasts with bounded MPSC channels.
- Session logs now use buffered writes and an in-memory 1 MiB live view.
- `PortChannelData::PortOpen` now carries the complete connection settings.
- `Serial::error` now records a user-visible reason.

### Removed

- Unused mock LLM types and UI, incomplete encoding variants, parsing-file APIs,
  and other dead public interfaces.
- Unused dependencies and the unreferenced 10.5 MiB wallpaper asset.

### Compatibility

This release intentionally contains public API changes and therefore advances
the pre-1.0 minor version from 0.1 to 0.2.

[0.2.0]: https://github.com/AnlangA/serial_bevy/compare/v0.1.0...HEAD
