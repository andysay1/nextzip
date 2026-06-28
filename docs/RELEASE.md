# Release Checklist

Recommended tag for the current state:

```text
v0.1.0-alpha
```

Run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
python3 scripts/generate_corpus.py --rows 100000
python3 scripts/run_benchmarks.py
target/release/nextzip bench benchmarks/data --json benchmarks/results/results.json
```

Smoke test:

```bash
target/release/nextzip pack benchmarks/data/telemetry.jsonl /tmp/telemetry.nxz
target/release/nextzip unpack /tmp/telemetry.nxz /tmp/telemetry.jsonl
cmp -s benchmarks/data/telemetry.jsonl /tmp/telemetry.jsonl
target/release/nextzip inspect /tmp/telemetry.nxz
target/release/nextzip bench benchmarks/data --json benchmarks/results/results.json
```

Publish artifacts:

- `target/release/nextzip`
- `README.md`
- `CHANGELOG.md`
- `LICENSE`
- `docs/`

Licensing:

- Public repository license: `PolyForm-Noncommercial-1.0.0`
- Commercial users need a separate paid commercial license agreement.
