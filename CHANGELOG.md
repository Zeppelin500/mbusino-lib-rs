# mbusino-lib-rs change log

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [0.1.0] - 2026-11-11

### Added

- Initial port of **MBusinoLib** (0.11.0) to Rust: the `Decoder` struct with
  `decode_records`, `decode_header` and `decode_telegram`, plus the `Record`,
  `Header`, `Frame` and `Telegram` types and the lookup/VIF helpers.
- Lookup tables generated from MBusinoLibPy (`tools/gen_tables.py`).
- Dependency-free JSON writer; golden test suite from the C++ reference
  output.
