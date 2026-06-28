use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::checksum::blake3_bytes;
use crate::entropy::zstd;
use crate::formats::table::StoredTable;
use crate::header::{decode_header, encode_header, ArchiveHeader, InputFormat, MAGIC, VERSION};

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

pub fn pack_file(input: &Path, output: &Path, options: PackOptions) -> anyhow::Result<()> {
    let mut file = File::open(input).with_context(|| format!("open {}", input.display()))?;
    let mut sample = vec![0; 64 * 1024];
    let sample_len = file
        .read(&mut sample)
        .with_context(|| format!("sample {}", input.display()))?;
    sample.truncate(sample_len);
    let format = crate::detect_format(&sample);

    if format == InputFormat::BinaryFallback {
        return pack_fallback_file(input, output, format, options);
    }

    let bytes = std::fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let archive = pack(&bytes, options)?;
    std::fs::write(output, archive).with_context(|| format!("write {}", output.display()))?;
    Ok(())
}

pub fn unpack_file(input: &Path, output: &Path) -> anyhow::Result<()> {
    let mut input_file = File::open(input).with_context(|| format!("open {}", input.display()))?;
    let (header, payload_len) = read_archive_header(&mut input_file)?;
    if !header.fallback_used {
        let bytes = std::fs::read(input).with_context(|| format!("read {}", input.display()))?;
        let restored = unpack(&bytes)?;
        std::fs::write(output, restored).with_context(|| format!("write {}", output.display()))?;
        return Ok(());
    }

    let (restored_size, output_hash, out) = {
        let mut payload_reader = std::io::Read::by_ref(&mut input_file).take(payload_len);
        let mut decoder = ::zstd::stream::read::Decoder::new(&mut payload_reader)
            .context("open payload decoder")?;
        let output_dir = output.parent().unwrap_or_else(|| Path::new("."));
        let mut out = tempfile::NamedTempFile::new_in(output_dir)
            .with_context(|| format!("create temporary output in {}", output_dir.display()))?;
        let mut hasher = blake3::Hasher::new();
        let mut restored_size = 0u64;
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let read = decoder.read(&mut buffer).context("decode payload")?;
            if read == 0 {
                break;
            }
            out.write_all(&buffer[..read])
                .with_context(|| format!("write {}", output.display()))?;
            hasher.update(&buffer[..read]);
            restored_size += read as u64;
        }
        out.flush()
            .with_context(|| format!("flush {}", output.display()))?;
        drop(decoder);
        std::io::copy(&mut payload_reader, &mut std::io::sink()).context("drain payload")?;

        (restored_size, *hasher.finalize().as_bytes(), out)
    };
    if restored_size != header.original_size || output_hash != header.original_hash {
        return Err(anyhow!("checksum mismatch"));
    }

    let mut checksum = [0u8; 32];
    input_file
        .read_exact(&mut checksum)
        .context("read archive checksum")?;
    if checksum != header.original_hash {
        return Err(anyhow!("archive checksum does not match header"));
    }
    out.persist(output)
        .map_err(|err| anyhow!("persist {}: {}", output.display(), err.error))?;
    Ok(())
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
    let parsed = parse_archive_envelope(bytes)?;
    let header_compressed_len = parsed.header_compressed.len();
    let payload_compressed_len = parsed.payload_compressed.len();
    let archive = archive_from_envelope(parsed)?;
    let blocks = if archive.header.fallback_used {
        0
    } else {
        crate::column::payload::block_count(&archive.payload).unwrap_or(0)
    };
    let ratio = if archive.header.original_size == 0 {
        0.0
    } else {
        bytes.len() as f64 / archive.header.original_size as f64
    };
    let mut out = format!(
        "version: {}\nheader_schema: {}\nformat: {:?}\noriginal_size: {} bytes\narchive_size: {} bytes\ncompression_ratio: {:.4}\nfallback: {}\nrows: {}\ncolumns: {}\nblocks: {}\nsize_breakdown:\n  header_compressed: {} bytes\n  payload_compressed: {} bytes\n  payload_decoded: {} bytes\n  container_overhead: {} bytes",
        archive.header.version,
        archive.header.header_schema_version,
        archive.header.format,
        archive.header.original_size,
        bytes.len(),
        ratio,
        archive.header.fallback_used,
        archive.header.row_count,
        archive.header.column_plans.len(),
        blocks,
        header_compressed_len,
        payload_compressed_len,
        archive.payload.len(),
        ARCHIVE_FIXED_OVERHEAD
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

    let header_bytes = zstd::encode(&encode_header(&header)?, options.level)?;
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
    let parsed = parse_archive_envelope(bytes)?;
    archive_from_envelope(parsed)
}

fn archive_from_envelope(parsed: ArchiveEnvelope<'_>) -> anyhow::Result<Archive> {
    let header = decode_header(&zstd::decode(parsed.header_compressed)?)?;
    let payload = zstd::decode(parsed.payload_compressed)?;
    if parsed.checksum != header.original_hash {
        return Err(anyhow!("archive checksum does not match header"));
    }
    Ok(Archive { header, payload })
}

struct ArchiveEnvelope<'a> {
    header_compressed: &'a [u8],
    payload_compressed: &'a [u8],
    checksum: [u8; 32],
}

const ARCHIVE_FIXED_OVERHEAD: usize = 4 + 4 + 4 + 8 + 8 + 32;

fn parse_archive_envelope(bytes: &[u8]) -> anyhow::Result<ArchiveEnvelope<'_>> {
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
    let header_start = cursor.position() as usize;
    let header_end = header_start
        .checked_add(header_len)
        .ok_or_else(|| anyhow!("header length overflow"))?;
    if header_end > bytes.len() {
        return Err(anyhow!("truncated header"));
    }
    cursor.set_position(header_end as u64);
    let payload_len = cursor.read_u64::<LittleEndian>()? as usize;
    let payload_start = cursor.position() as usize;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or_else(|| anyhow!("payload length overflow"))?;
    if payload_end > bytes.len() {
        return Err(anyhow!("truncated payload"));
    }
    cursor.set_position(payload_end as u64);
    let mut checksum = [0u8; 32];
    cursor.read_exact(&mut checksum).context("read checksum")?;
    Ok(ArchiveEnvelope {
        header_compressed: &bytes[header_start..header_end],
        payload_compressed: &bytes[payload_start..payload_end],
        checksum,
    })
}

fn read_archive_header(reader: &mut impl Read) -> anyhow::Result<(ArchiveHeader, u64)> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(anyhow!("bad magic"));
    }
    let version = reader.read_u32::<LittleEndian>()?;
    if version != VERSION {
        return Err(anyhow!("unsupported version {version}"));
    }
    let _flags = reader.read_u32::<LittleEndian>()?;
    let header_len = reader.read_u64::<LittleEndian>()?;
    let mut header_compressed = vec![0u8; header_len as usize];
    reader
        .read_exact(&mut header_compressed)
        .context("read header")?;
    let header = decode_header(&zstd::decode(&header_compressed)?)?;
    let payload_len = reader.read_u64::<LittleEndian>()?;
    Ok((header, payload_len))
}

fn pack_fallback_file(
    input: &Path,
    output: &Path,
    format: InputFormat,
    options: PackOptions,
) -> anyhow::Result<()> {
    let (original_size, original_hash) = hash_file(input)?;
    let mut payload_tmp = tempfile::tempfile().context("create temporary payload")?;
    let mut input_file = File::open(input).with_context(|| format!("open {}", input.display()))?;
    ::zstd::stream::copy_encode(&mut input_file, &mut payload_tmp, options.level)
        .context("encode fallback payload")?;
    let payload_len = payload_tmp
        .seek(SeekFrom::End(0))
        .context("measure payload")?;
    payload_tmp
        .seek(SeekFrom::Start(0))
        .context("rewind payload")?;

    let header = ArchiveHeader {
        version: VERSION,
        header_schema_version: 1,
        original_size,
        original_hash,
        format,
        exact_mode: options.exact,
        fallback_used: true,
        schema: Default::default(),
        row_count: 0,
        column_plans: Vec::new(),
    };
    let header_bytes = zstd::encode(&encode_header(&header)?, options.level)?;

    let mut out = File::create(output).with_context(|| format!("create {}", output.display()))?;
    out.write_all(MAGIC)?;
    out.write_u32::<LittleEndian>(VERSION)?;
    out.write_u32::<LittleEndian>(if options.exact { 1 } else { 0 })?;
    out.write_u64::<LittleEndian>(header_bytes.len() as u64)?;
    out.write_all(&header_bytes)?;
    out.write_u64::<LittleEndian>(payload_len)?;
    std::io::copy(&mut payload_tmp, &mut out).context("write fallback payload")?;
    out.write_all(&header.original_hash)?;
    Ok(())
}

fn hash_file(path: &Path) -> anyhow::Result<(u64, [u8; 32])> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((size, *hasher.finalize().as_bytes()))
}
