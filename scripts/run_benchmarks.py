#!/usr/bin/env python3
from __future__ import annotations

import argparse
import gzip
import json
import shutil
import subprocess
import time
from pathlib import Path


def run(cmd):
    started = time.perf_counter()
    result = subprocess.run(cmd, check=True, capture_output=True)
    return result, time.perf_counter() - started


def zstd_size(path: Path, level: int) -> int:
    proc = subprocess.run(["zstd", f"-{level}", "-q", "-c", str(path)], check=True, capture_output=True)
    return len(proc.stdout)


def gzip_size(path: Path) -> int:
    return len(gzip.compress(path.read_bytes(), compresslevel=6))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", type=Path, default=Path("target/release/nextzip"))
    parser.add_argument("--data", type=Path, default=Path("benchmarks/data"))
    parser.add_argument("--out", type=Path, default=Path("benchmarks/results/results.json"))
    parser.add_argument("--level", type=int, default=3)
    args = parser.parse_args()
    args.out.parent.mkdir(parents=True, exist_ok=True)

    if not args.bin.exists():
        subprocess.run(["cargo", "build", "--release"], check=True)

    zstd_available = shutil.which("zstd") is not None
    rows = []
    for path in sorted(args.data.iterdir()):
        if not path.is_file():
            continue
        archive = args.out.parent / f"{path.name}.nxz"
        restored = args.out.parent / f"{path.name}.restored"

        _, pack_seconds = run([str(args.bin), "pack", str(path), str(archive), "--level", str(args.level)])
        _, unpack_seconds = run([str(args.bin), "unpack", str(archive), str(restored)])
        diff = subprocess.run(["cmp", "-s", str(path), str(restored)])
        inspect = subprocess.run([str(args.bin), "inspect", str(archive)], check=True, capture_output=True, text=True).stdout

        original = path.stat().st_size
        nxz = archive.stat().st_size
        gz = gzip_size(path)
        zs = zstd_size(path, args.level) if zstd_available else None

        rows.append(
            {
                "file": path.name,
                "original": original,
                "nextzip": nxz,
                "gzip": gz,
                "zstd": zs,
                "ratio_vs_zstd": (zs / nxz) if zs else None,
                "ratio_vs_gzip": gz / nxz,
                "pack_seconds": pack_seconds,
                "unpack_seconds": unpack_seconds,
                "roundtrip_ok": diff.returncode == 0,
                "fallback": "fallback: true" in inspect,
                "inspect": inspect,
            }
        )

    args.out.write_text(json.dumps(rows, indent=2), encoding="utf-8")
    print_markdown(rows)


def fmt_size(value):
    if value is None:
        return "n/a"
    for unit in ["B", "KB", "MB", "GB"]:
        if value < 1024:
            return f"{value:.0f} {unit}"
        value /= 1024
    return f"{value:.0f} TB"


def print_markdown(rows):
    print("| file | original | nextzip | zstd | gzip | vs zstd | fallback | roundtrip |")
    print("|---|---:|---:|---:|---:|---:|---|---|")
    for row in rows:
        ratio = "n/a" if row["ratio_vs_zstd"] is None else f'{row["ratio_vs_zstd"]:.2f}x'
        print(
            f'| {row["file"]} | {fmt_size(row["original"])} | {fmt_size(row["nextzip"])} | '
            f'{fmt_size(row["zstd"])} | {fmt_size(row["gzip"])} | {ratio} | '
            f'{row["fallback"]} | {row["roundtrip_ok"]} |'
        )


if __name__ == "__main__":
    main()
