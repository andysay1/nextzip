use std::io::{Cursor, Read};

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::checksum::blake3_bytes;
use crate::entropy::zstd;
use crate::formats::table::StoredTable;
use crate::header::{ArchiveHeader, InputFormat, MAGIC, VERSION};

#[derive(Debug, Clone, Copy)]
pub struct PackOptions {
    pub exact: bool,
    pub level: i32,
}

#[derive(Debug)]
pub struct Archive {
    pub header: ArchiveHeader,
    pub payload: Vec<u8>,
}

pub fn pack(input: &[u8], options: PackOptions) -> anyhow::Result<Vec<u8>> {
    let format = crate::detect_format(input);
    let fallback = build_archive(input, format, true, options, input.to_vec(), None)?;

    if format == InputFormat::BinaryFallback {
        return Ok(fallback);
    }

    let structural = build_structural(input, format, options)
        .and_then(|archive| {
            let restored = unpack(&archive)?;
            if restored == input {
                Ok(archive)
            } else {
                Err(anyhow!("structural candidate is not byte-exact"))
            }
        })
        .ok();

    match structural {
        Some(candidate) if candidate.len() < fallback.len() => Ok(candidate),
        _ => Ok(fallback),
    }
}

pub fn unpack(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let archive = parse_archive(bytes)?;
    let output = if archive.header.fallback_used {
        archive.payload
    } else {
        let table = crate::column::payload::decode_table(&archive.payload)?;
        match archive.header.format {
            InputFormat::Jsonl => crate::formats::jsonl::reconstruct(&table)?,
            InputFormat::Csv | InputFormat::Tsv => crate::formats::csv::reconstruct(&table)?,
            InputFormat::Logs => crate::formats::logs::reconstruct(&table)?,
            InputFormat::BinaryFallback => archive.payload,
        }
    };

    if blake3_bytes(&output) != archive.header.original_hash {
        return Err(anyhow!("checksum mismatch"));
    }
    Ok(output)
}

pub fn inspect_archive(bytes: &[u8]) -> anyhow::Result<String> {
    let archive = parse_archive(bytes)?;
    let blocks = if archive.header.fallback_used {
        0
    } else {
        crate::column::payload::block_count(&archive.payload).unwrap_or(0)
    };
    let mut out = format!(
        "version: {}\nheader_schema: {}\nformat: {:?}\noriginal_size: {} bytes\nfallback: {}\nrows: {}\ncolumns: {}\nblocks: {}",
        archive.header.version,
        archive.header.header_schema_version,
        archive.header.format,
        archive.header.original_size,
        archive.header.fallback_used,
        archive.header.row_count,
        archive.header.column_plans.len(),
        blocks
    );
    for plan in &archive.header.column_plans {
        out.push_str(&format!(
            "\n- {}: {:?}, {:?}, encoded_estimate={} bytes",
            plan.name, plan.column_type, plan.codec, plan.encoded_len_estimate
        ));
    }
    if !archive.header.fallback_used {
        let stats = crate::column::payload::codec_stats(&archive.payload)?;
        if !stats.is_empty() {
            out.push_str("\nblock_codec_stats:");
            for stat in stats {
                out.push_str(&format!(
                    "\n  - {}#{} {:?}: chunks={}, bytes={}",
                    stat.column_name, stat.column_id, stat.codec, stat.chunks, stat.bytes
                ));
            }
        }
    }
    Ok(out)
}

fn build_structural(
    input: &[u8],
    format: InputFormat,
    options: PackOptions,
) -> anyhow::Result<Vec<u8>> {
    let table = match format {
        InputFormat::Jsonl if options.exact => crate::formats::jsonl::parse_exact(input)?,
        InputFormat::Jsonl => crate::formats::jsonl::parse(input)?,
        InputFormat::Csv => crate::formats::csv::parse(input)?,
        InputFormat::Tsv => crate::formats::tsv::parse(input)?,
        InputFormat::Logs => crate::formats::logs::parse(input)?,
        InputFormat::BinaryFallback => {
            return Err(anyhow!("binary fallback has no structural plan"))
        }
    };
    build_archive(input, format, false, options, Vec::new(), Some(table))
}

fn build_archive(
    input: &[u8],
    format: InputFormat,
    fallback_used: bool,
    options: PackOptions,
    payload: Vec<u8>,
    table: Option<StoredTable>,
) -> anyhow::Result<Vec<u8>> {
    let (schema, column_plans, row_count) = if let Some(table) = table.as_ref() {
        let (schema, plans) = table.schema_and_plans();
        (schema, plans, table.row_count as u64)
    } else {
        (Default::default(), Vec::new(), 0)
    };
    let payload = if fallback_used {
        payload
    } else {
        let table = table.context("structural archive needs table payload")?;
        crate::column::payload::encode_table(&table, &column_plans)?
    };
    let header = ArchiveHeader {
        version: VERSION,
        header_schema_version: 1,
        original_size: input.len() as u64,
        original_hash: blake3_bytes(input),
        format,
        exact_mode: options.exact,
        fallback_used,
        schema,
        row_count,
        column_plans,
    };

    let header_bytes = zstd::encode(&bincode::serialize(&header)?, options.level)?;
    let payload_bytes = zstd::encode(&payload, options.level)?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.write_u32::<LittleEndian>(VERSION)?;
    out.write_u32::<LittleEndian>(if options.exact { 1 } else { 0 })?;
    out.write_u64::<LittleEndian>(header_bytes.len() as u64)?;
    out.extend_from_slice(&header_bytes);
    out.write_u64::<LittleEndian>(payload_bytes.len() as u64)?;
    out.extend_from_slice(&payload_bytes);
    out.extend_from_slice(&blake3_bytes(input));
    Ok(out)
}

fn parse_archive(bytes: &[u8]) -> anyhow::Result<Archive> {
    let mut cursor = Cursor::new(bytes);
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(anyhow!("bad magic"));
    }
    let version = cursor.read_u32::<LittleEndian>()?;
    if version != VERSION {
        return Err(anyhow!("unsupported version {version}"));
    }
    let _flags = cursor.read_u32::<LittleEndian>()?;
    let header_len = cursor.read_u64::<LittleEndian>()? as usize;
    let mut header_compressed = vec![0; header_len];
    cursor.read_exact(&mut header_compressed)?;
    let header: ArchiveHeader = bincode::deserialize(&zstd::decode(&header_compressed)?)?;
    if header.header_schema_version != 1 {
        return Err(anyhow!(
            "unsupported header schema {}",
            header.header_schema_version
        ));
    }
    let payload_len = cursor.read_u64::<LittleEndian>()? as usize;
    let mut payload_compressed = vec![0; payload_len];
    cursor
        .read_exact(&mut payload_compressed)
        .context("read payload")?;
    let payload = zstd::decode(&payload_compressed)?;
    let mut checksum = [0u8; 32];
    cursor.read_exact(&mut checksum).context("read checksum")?;
    if checksum != header.original_hash {
        return Err(anyhow!("archive checksum does not match header"));
    }
    Ok(Archive { header, payload })
}
