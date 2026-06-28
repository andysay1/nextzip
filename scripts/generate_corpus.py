#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
import random
from pathlib import Path


def write_jsonl(path: Path, rows):
    with path.open("w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, separators=(",", ":"), sort_keys=True))
            f.write("\n")


def telemetry_rows(n: int):
    regions = ["us-east", "us-west", "eu", "apac", "latam"]
    events = ["page_view", "click", "purchase", "search", "add_to_cart"]
    devices = ["ios", "android", "web"]
    for i in range(n):
        yield {
            "ts": 1_710_000_000 + i,
            "user_id": 100_000 + (i % 7_919),
            "session_id": f"s-{i // 18:08d}",
            "event": events[(i * 17) % len(events)],
            "region": regions[(i * 7) % len(regions)],
            "device": devices[(i * 11) % len(devices)],
            "latency_ms": 40 + ((i * 13) % 900),
            "price_cents": 0 if i % 9 else 499 + ((i * 19) % 20_000),
        }


def session_rows(n: int):
    actions = ["open", "scroll", "click", "like", "share", "close"]
    countries = ["US", "DE", "FR", "JP", "BR", "IN", "CA"]
    for i in range(n):
        yield {
            "ts": 1_720_000_000 + i * 3 + (1 if i % 97 == 0 else 0),
            "account": 10_000 + (i % 1_337),
            "country": countries[(i * 5) % len(countries)],
            "action": actions[(i * 3 + i // 50) % len(actions)],
            "screen": f"screen_{(i // 12) % 23}",
            "duration_ms": 150 + ((i * i) % 12_000),
        }


def mixed_rows(n: int):
    random.seed(42)
    levels = ["debug", "info", "warn", "error"]
    for i in range(n):
        row = {
            "ts": 1_730_000_000 + i,
            "level": levels[i % len(levels)],
            "request_id": f"req-{i:08x}",
            "status": [200, 200, 200, 201, 400, 404, 500][i % 7],
            "message": "cache hit" if i % 3 else f"slow query shard={i % 31}",
        }
        if i % 11 == 0:
            row["debug_payload"] = "".join(random.choice("abcdef0123456789") for _ in range(48))
        yield row


def write_sales_csv(path: Path, n: int):
    products = ["book", "laptop", "phone", "chair", "desk", "camera", "shoe"]
    channels = ["web", "retail", "partner"]
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(["ts", "order_id", "customer_id", "product", "channel", "qty", "amount_cents"])
        for i in range(n):
            writer.writerow(
                [
                    1_715_000_000 + i * 17,
                    5_000_000 + i,
                    300_000 + (i % 16_384),
                    products[(i * 13) % len(products)],
                    channels[(i * 5) % len(channels)],
                    1 + (i % 4),
                    599 + ((i * 29) % 250_000),
                ]
            )


def write_sales_realistic_csv(path: Path, n: int):
    random.seed(1234)
    products = [
        ("book", 1299),
        ("laptop", 119900),
        ("phone", 79900),
        ("chair", 9900),
        ("desk", 21900),
        ("camera", 54900),
        ("shoe", 8990),
        ("coffee", 1699),
        ("monitor", 32900),
    ]
    channels = ["web", "retail", "partner", "marketplace"]
    countries = ["US", "DE", "FR", "JP", "BR", "IN", "CA", "GB", "AU"]
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(
            [
                "ts",
                "order_id",
                "customer_id",
                "country",
                "product",
                "channel",
                "qty",
                "discount_bps",
                "amount_cents",
            ]
        )
        ts = 1_715_000_000
        for i in range(n):
            product, base_price = random.choice(products)
            qty = random.choices([1, 2, 3, 4, 5], weights=[70, 16, 8, 4, 2])[0]
            discount = random.choice([0, 0, 0, 250, 500, 1000, 1500])
            jitter = random.randint(-base_price // 20, base_price // 20)
            amount = max(99, (base_price + jitter) * qty * (10_000 - discount) // 10_000)
            ts += random.randint(1, 90)
            writer.writerow(
                [
                    ts,
                    7_000_000 + i + random.randint(0, 7),
                    300_000 + random.randrange(0, 60_000),
                    random.choice(countries),
                    product,
                    random.choice(channels),
                    qty,
                    discount,
                    amount,
                ]
            )


def write_logs(path: Path, n: int):
    levels = ["INFO", "INFO", "INFO", "WARN", "ERROR"]
    actions = ["login", "view", "buy", "logout", "search"]
    with path.open("w", encoding="utf-8") as f:
        for i in range(n):
            day = 1 + (i // 86_400)
            f.write(
                f"2026-01-{day:02d}T12:{(i // 60) % 60:02d}:{i % 60:02d}Z "
                f"{levels[i % len(levels)]} user={1000 + i % 4096} "
                f"action={actions[(i * 7) % len(actions)]} item={9000 + i % 512} "
                f"latency={25 + (i * 11) % 700}\n"
            )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=Path("benchmarks/data"))
    parser.add_argument("--rows", type=int, default=100_000)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    write_jsonl(args.out / "telemetry.jsonl", telemetry_rows(args.rows))
    write_jsonl(args.out / "sessions.jsonl", session_rows(args.rows))
    write_jsonl(args.out / "mixed.jsonl", mixed_rows(args.rows))
    write_sales_csv(args.out / "sales.csv", args.rows)
    write_sales_realistic_csv(args.out / "sales_realistic.csv", args.rows)
    write_logs(args.out / "app.log", args.rows)

    random.seed(7)
    (args.out / "random.bin").write_bytes(bytes(random.randrange(0, 256) for _ in range(2_000_000)))


if __name__ == "__main__":
    main()
