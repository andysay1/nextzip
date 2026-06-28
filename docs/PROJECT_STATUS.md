# Project Status

## Implemented

- CLI: `pack`, `unpack`, `inspect`, `bench`.
- Archive container with `NXZ1`, compressed header, compressed payload, and
  Blake3 checksum.
- Format detection for JSONL, CSV, TSV, logs, and binary fallback.
- Byte-for-byte verification before accepting structural archives.
- Row-block structural payload with independent per-block column codec choices.
- Manual binary column chunks without bincode enum overhead.
- Packed presence bitmaps.
- String dictionary with bitpacked indexes.
- Integer codecs: delta, delta-of-delta, bitpack, frame-of-reference.
- RLE and raw chunks.
- Reproducible benchmark corpus and runner.
- CSV CRLF line endings and header order preservation.
- Log template extraction for `timestamp LEVEL key=value ...` streams.
- Mixed log template field-order preservation.
- `inspect` block-level codec statistics.
- Directory benchmark mode with optional JSON output.
- Versioned header schema.
- Property-style, CSV dialect, and corrupt archive tests.
- CI workflow and release checklist.
- JSONL `--exact` structural raw-line residual path.

## Current Strengths

- Machine-generated JSONL.
- Telemetry/session exports.
- Repeated log-like streams.
- Template-style application logs.
- CSV exports with stable columns and exact line-ending preservation.
- Correct fallback behavior on high-entropy binary data.

## Known Gaps

- CSV quoting style and escape-style preservation is incomplete for unusual
  dialects; such cases remain protected by fallback and byte-for-byte self-test.
- JSONL `--exact` has a structural raw-line residual path; a future
  `canonical_json + formatting_delta` path should reduce exact-mode size.
- Logs preserve mixed `key=value` field order; future work can add explicit
  template IDs and per-template column groups for smaller payloads.
- Header and payload chunks use manual binary records.
