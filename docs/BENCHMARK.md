# Benchmark Methodology

Benchmarks are generated with deterministic scripts so results are reproducible
without downloading private or unstable datasets.

## Corpus

Generate data:

```bash
python3 scripts/generate_corpus.py --rows 100000
```

Files:

- `telemetry.jsonl`: event telemetry with timestamps, low-cardinality strings,
  sessions, IDs, and metrics.
- `sessions.jsonl`: user-session stream with durations and categorical fields.
- `mixed.jsonl`: structured rows with occasional high-entropy debug payloads.
- `sales.csv`: numeric and categorical order data.
- `sales_realistic.csv`: seeded random order data with categorical fields,
  jittered prices, discounts, and non-linear timestamps.
- `app.log`: repeated log template style data.
- `random.bin`: high-entropy binary fallback case.

Run benchmarks:

```bash
cargo build --release
python3 scripts/run_benchmarks.py
```

The runner verifies `unpack(pack(x)) == x` with `cmp -s`.

## Reading Results

`nextzip` should beat gzip/zstd on structured machine data and use fallback on
random/high-entropy binary data. Extremely regular synthetic streams may produce
very large ratios; mixed datasets are more representative of ordinary exports.

See `docs/BENCHMARK_RESULTS.md` for the latest local run.
