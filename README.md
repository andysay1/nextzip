# NextZip-S

Structural archive MVP written in Rust. Current release target:
`0.1.0-alpha`.

## License

NextZip-S is source-available under the PolyForm Noncommercial License 1.0.0.
Personal, research, educational, and other noncommercial use is permitted.
Commercial use requires a separate commercial license from the project owner.

## Commands

```bash
cargo run -- pack input.jsonl output.nxz
cargo run -- unpack output.nxz restored.jsonl
cargo run -- inspect output.nxz
cargo run -- bench input.jsonl
```

The archive format uses `NXZ1` magic, a zstd-compressed bincode header, a
zstd-compressed payload, and a Blake3 checksum of the original input. The
structural payload is a manual binary block/column format with packed bitmaps,
dictionary indexes, delta, delta-of-delta, bitpack, frame-of-reference, RLE, and
raw chunks.

Packing tries a structural plan for JSONL, CSV, TSV, and logs, verifies
byte-for-byte decode, and falls back to zstd when the structural archive is not
smaller or not exact.

## Validation

```bash
cargo test
cargo build --release
```

Release hardening checks:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Documentation

- `docs/FORMAT.md` describes the current container and payload layout.
- `docs/BENCHMARK.md` describes the reproducible benchmark corpus.
- `docs/BENCHMARK_RESULTS.md` contains the latest local benchmark results.
- `docs/PROJECT_STATUS.md` summarizes implemented features and known gaps.
- `docs/RELEASE.md` contains the release checklist.

## Benchmarks

```bash
python3 scripts/generate_corpus.py --rows 100000
python3 scripts/run_benchmarks.py
```
