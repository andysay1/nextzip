# NextZip-S Archive Format

## Container

```text
MAGIC        4 bytes     NXZ1
VERSION      u32 LE
FLAGS        u32 LE
HEADER_LEN   u64 LE
HEADER       zstd(bincode(ArchiveHeader))
PAYLOAD_LEN  u64 LE
PAYLOAD      zstd(binary payload)
CHECKSUM     32 bytes    blake3(original)
```

The public header is still serialized with bincode for compatibility with the
Rust structures. It carries `header_schema_version = 1` so alpha archives can be
rejected or migrated as the format changes. Column payloads are manual binary
records.

## Structural Payload

```text
FORMAT_ID          u8
DELIMITER_PRESENT  u8
DELIMITER          u8 if present
TRAILING_NEWLINE   u8
ROW_COUNT          u64
COLUMN_COUNT       u32
COLUMN_NAME[]      len:u64 + utf8 bytes
BLOCK_COUNT        u32

for each block:
  BLOCK_ROWS       u32
  for each column:
    COLUMN_ID      u32
    CODEC_ID       u8
    CHUNK_LEN      u64
    CHUNK_BYTES
```

Each block chooses codecs independently. Current target block size is 16,384
rows.

`inspect` reports both header-level column plans and actual block-level codec
statistics, because each block may choose a different codec for the same column.

## Column Chunks

All nullable codecs use packed presence bitmaps.

- `Dictionary`: string table plus bitpacked dictionary indexes.
- `Delta`: packed presence plus zigzag varint deltas.
- `DeltaOfDelta`: packed presence plus base, first delta, and zigzag varint
  second-order deltas.
- `BitPack`: packed unsigned integers.
- `FrameOfReference`: base plus bitpacked offsets.
- `Rle`: run count plus repeated cell values.
- `Raw`: typed cells with explicit presence markers.

## Exact JSONL Residual

When `pack --exact` is used for JSONL and the structural candidate beats
fallback, the payload may store a special `__nxz_raw_line` dictionary/RLE column.
This preserves original field order, whitespace, and line endings while still
allowing entropy compression and block-level structural planning.

## Log Template Residual

Template-style logs are represented as semantic columns such as `timestamp`,
`level`, `user`, `action`, and `latency`. Mixed templates preserve exact
per-line field order with a special `__nxz_field_order` column, allowing files
with different `key=value` layouts to roundtrip byte-for-byte.
