# Changelog

## 0.1.0-alpha - 2026-06-28

Initial technical preview.

### Added

- CLI commands: `pack`, `unpack`, `inspect`, `bench`.
- `NXZ1` archive container with versioned bincode header schema and manual
  binary structural payload.
- JSONL, CSV, TSV, log, and binary fallback detection.
- Byte-for-byte verification before accepting structural archives.
- Row-block payload with per-block column codec selection.
- Codecs: dictionary, delta, delta-of-delta, RLE, bitpack,
  frame-of-reference, raw.
- Packed presence bitmaps and compact string dictionary indexes.
- CSV CRLF/LF and header-order preservation.
- Template-style log extraction for `timestamp LEVEL key=value ...` streams.
- JSONL `--exact` structural raw-line residual path when it beats fallback.
- Reproducible benchmark corpus and benchmark runner.
- Property-style, dialect, corruption, unit, and integration tests.
- CI workflow for fmt, clippy, tests, release build, and benchmark smoke.
- PolyForm Noncommercial licensing for free noncommercial use and separate
  commercial licensing.

### Known Limitations

- CSV unusual quoting/escape dialects may still fall back.
- Logs support one common template family; mixed multi-template logs need more
  work.
- Header is versioned but still bincode encoded in this alpha.
