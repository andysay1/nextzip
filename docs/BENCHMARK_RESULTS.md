# Benchmark Results

Generated on the local machine with:

```bash
cargo build --release
python3 scripts/generate_corpus.py --rows 100000
python3 scripts/run_benchmarks.py
```

The runner performs:

```text
nextzip pack input output.nxz
nextzip unpack output.nxz restored
cmp -s input restored
```

All roundtrips below are byte-for-byte.

## Results

| file | type | original | nextzip | zstd | gzip | ratio vs zstd | fallback | roundtrip |
|---|---|---:|---:|---:|---:|---:|---|---|
| `app.log` | logs | 7,089,282 | 19,637 | 908,902 | 1,050,310 | 46.29x | false | true |
| `mixed.jsonl` | JSONL mixed entropy | 10,581,683 | 282,595 | 1,207,397 | 1,064,680 | 4.27x | false | true |
| `random.bin` | high entropy binary | 2,000,000 | 2,000,179 | 2,000,061 | 2,000,633 | 1.00x | true | true |
| `sales.csv` | highly regular CSV | 4,773,914 | 1,592 | 1,212,083 | 1,335,595 | 761.36x | false | true |
| `sales_realistic.csv` | seeded realistic CSV | 5,523,377 | 816,570 | 1,819,072 | 1,570,350 | 2.23x | false | true |
| `sessions.jsonl` | JSONL sessions | 10,545,929 | 9,909 | 1,207,002 | 1,133,594 | 121.81x | false | true |
| `telemetry.jsonl` | JSONL telemetry | 14,645,551 | 35,716 | 718,708 | 1,015,700 | 20.12x | false | true |

## Observations

- Structured JSONL and logs are the current sweet spot.
- `random.bin` correctly uses fallback and does not pretend to compress entropy.
- `sales.csv` is intentionally highly regular and demonstrates the best case for
  structural program compression.
- `sales_realistic.csv` is the more useful CSV signal: seeded random orders,
  non-linear timestamps, categorical columns, jittered amounts, and exact CRLF
  preservation. It still beats zstd by 2.23x.
- Extremely high ratios on `sessions.jsonl` and `telemetry.jsonl` come from
  repeated program-like structure: low-cardinality strings, monotonic timestamps,
  bounded IDs, and repeated session/event patterns.

## Timings

| file | pack seconds | unpack seconds |
|---|---:|---:|
| `app.log` | 0.577 | 0.034 |
| `mixed.jsonl` | 0.332 | 0.056 |
| `random.bin` | 0.006 | 0.005 |
| `sales.csv` | 0.119 | 0.041 |
| `sales_realistic.csv` | 0.229 | 0.050 |
| `sessions.jsonl` | 0.281 | 0.054 |
| `telemetry.jsonl` | 0.376 | 0.072 |

## Next Engineering Fixes

1. Preserve more CSV dialect details: quoting style, escapes, and empty-field
   distinctions.
2. Add multi-template log support for files that mix several log formats.
3. Add a public benchmark command that runs the whole corpus from the binary.
4. Add property/fuzz tests for JSONL/CSV/log roundtrips.
