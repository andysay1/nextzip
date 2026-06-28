# Changelog

## 0.1.0-alpha - 2026-06-28

Initial technical preview.

### Added

- CLI commands: `pack`, `unpack`, `inspect`, `bench`.
- `NXZ1` archive container with versioned manual binary header schema and manual
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
- Mixed log template field-order residuals.
- `inspect` text and JSON reports with size breakdowns and block-level codec
  statistics.
- Directory benchmark mode with optional JSON output.
- Streaming file pack/unpack path for binary fallback payloads.
- Reproducible benchmark corpus and benchmark runner.
- Property-style, dialect, corruption, unit, and integration tests.
- CI workflow for fmt, clippy, tests, release build, and benchmark smoke.
- PolyForm Noncommercial licensing for free noncommercial use and separate
  commercial licensing.

### Known Limitations

- CSV unusual quoting/escape dialects may still fall back.
- Logs can preserve mixed field order, but explicit template IDs and
  per-template column groups are future work.
- Structural JSONL/CSV/log paths still build an intermediate table in memory.
