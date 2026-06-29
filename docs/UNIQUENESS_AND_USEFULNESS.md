# NextZip-S Uniqueness and Usefulness Evaluation

Date: 2026-06-29

## Local Validation

Commands run:

```bash
cargo test
cargo build --release
python3 scripts/run_benchmarks.py
target/release/nextzip bench benchmarks/data --json /tmp/nextzip_eval_results.json
```

Result:

- 37 Rust tests passed.
- Release build passed.
- All benchmark corpus roundtrips passed.

## Compression Results

| file | type | original | nextzip | zstd | gzip | vs zstd | fallback |
|---|---|---:|---:|---:|---:|---:|---|
| app.log | logs | 7,089,282 | 19,472 | 920,607 | 1,022,644 | 47.28x | false |
| mixed.jsonl | JSONL | 10,581,683 | 282,577 | 1,217,388 | 1,032,195 | 4.31x | false |
| random.bin | binary | 2,000,000 | 2,000,179 | 2,000,054 | 2,000,338 | 1.00x | true |
| sales.csv | CSV | 4,773,914 | 1,579 | 1,224,832 | 1,339,283 | 775.70x | false |
| sales_realistic.csv | CSV | 5,523,377 | 816,555 | 1,824,216 | 1,572,315 | 2.23x | false |
| sessions.jsonl | JSONL | 10,545,929 | 9,905 | 1,203,507 | 1,060,641 | 121.50x | false |
| telemetry.jsonl | JSONL | 14,645,551 | 35,694 | 740,347 | 1,005,509 | 20.74x | false |

Aggregate:

- Total corpus: 55,159,736 bytes.
- NextZip-S total: 3,165,961 bytes.
- zstd total: 9,130,951 bytes.
- Overall: NextZip-S is 2.88x smaller than zstd on this mixed corpus.
- Structural files only: NextZip-S is 6.12x smaller than zstd.
- High-entropy binary fallback: NextZip-S is effectively equal to zstd, as intended.

## Speed Snapshot

Extended local run against `zstd -3`, `gzip -6`, `xz -6`, and `brotli -6` showed:

- NextZip-S encode is slower than zstd on structured data because it parses,
  plans, verifies, and writes structural payloads.
- NextZip-S decode is fast enough for the tested 5-15 MB files, typically
  around 0.04-0.07 seconds for structural benchmark files.
- zstd remains the right baseline for fastest general-purpose compression.
- xz and brotli can beat zstd on some sizes, but NextZip-S still wins strongly
  on the highly regular JSONL/CSV/log corpus.

## Comparison With Existing Formats

NextZip-S is not unique in using columnar ideas. Apache Parquet is an open
source column-oriented format with compression and encoding schemes for
efficient storage and retrieval:

https://parquet.apache.org/

Apache ORC is another mature columnar storage format with projection and
lightweight indexes:

https://orc.apache.org/specification/ORCv1/

Apache Arrow defines a language-independent columnar memory format and IPC/file
formats for efficient analytic operations:

https://arrow.apache.org/

zstd is a mature fast lossless compressor and remains the strongest baseline for
general byte-stream compression:

https://facebook.github.io/zstd/

## What Is Actually Unique

The unique angle is the product shape, not each individual technique:

- It targets source-like machine data files: JSONL, CSV/TSV, and logs.
- It preserves byte-for-byte restoration instead of converting data into an
  analytics format.
- It automatically chooses between structural compression and zstd fallback.
- It keeps archive semantics: `pack`, `unpack`, `inspect`, `bench`.
- It exposes structural diagnostics through text and JSON `inspect`.

In other words, it sits between general compressors and analytical columnar
formats.

## Usefulness Assessment

Strong use cases:

- Cold storage of logs, telemetry, sessions, exports, and repeated machine data.
- Developers who need exact original file restoration, not only table recovery.
- CI/data pipelines that want inspectable archive metadata and fallback safety.

Weak use cases:

- Arbitrary binary files.
- Already compressed files.
- Workloads where encode speed matters more than storage size.
- Big analytics ecosystems that already require Parquet/ORC for query pushdown.

## Verdict

Usefulness: high for a narrow but real domain: exact archival compression of
machine-generated text datasets.

Uniqueness: medium-high. The underlying primitives are known, but the archive
product shape is distinct: structural, byte-exact, CLI-first compression for
JSONL/CSV/logs with automatic fallback.

Release maturity: alpha. The core works and tests pass, but before calling it a
serious release the project still needs compatibility corpus tests, fuzzing,
streaming structural encoding, and release binaries.
