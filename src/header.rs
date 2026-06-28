use std::io::{Cursor, Read};

use anyhow::anyhow;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use crate::column::{ColumnCodec, ColumnPlan};
use crate::schema::{ColumnSchema, ColumnType, Schema};

pub const MAGIC: &[u8; 4] = b"NXZ1";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputFormat {
    Jsonl,
    Csv,
    Tsv,
    Logs,
    BinaryFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveHeader {
    pub version: u32,
    pub header_schema_version: u32,
    pub original_size: u64,
    pub original_hash: [u8; 32],
    pub format: InputFormat,
    pub exact_mode: bool,
    pub fallback_used: bool,
    pub schema: Schema,
    pub row_count: u64,
    pub column_plans: Vec<ColumnPlan>,
}

pub fn encode_header(header: &ArchiveHeader) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    out.write_u32::<LittleEndian>(header.version)?;
    out.write_u32::<LittleEndian>(header.header_schema_version)?;
    out.write_u64::<LittleEndian>(header.original_size)?;
    out.extend_from_slice(&header.original_hash);
    out.write_u8(format_id(header.format))?;
    out.write_u8(u8::from(header.exact_mode))?;
    out.write_u8(u8::from(header.fallback_used))?;
    out.write_u64::<LittleEndian>(header.row_count)?;

    out.write_u32::<LittleEndian>(header.schema.columns.len() as u32)?;
    for column in &header.schema.columns {
        write_column_schema(&mut out, column)?;
    }

    out.write_u32::<LittleEndian>(header.column_plans.len() as u32)?;
    for plan in &header.column_plans {
        write_string(&mut out, &plan.name)?;
        out.write_u8(column_type_id(&plan.column_type))?;
        out.write_u8(codec_id(plan.codec))?;
        out.write_u64::<LittleEndian>(plan.original_len)?;
        out.write_u64::<LittleEndian>(plan.encoded_len_estimate)?;
    }
    Ok(out)
}

pub fn decode_header(bytes: &[u8]) -> anyhow::Result<ArchiveHeader> {
    let mut cursor = Cursor::new(bytes);
    let version = cursor.read_u32::<LittleEndian>()?;
    let header_schema_version = cursor.read_u32::<LittleEndian>()?;
    if header_schema_version != 1 {
        return Err(anyhow!("unsupported header schema {header_schema_version}"));
    }
    let original_size = cursor.read_u64::<LittleEndian>()?;
    let mut original_hash = [0u8; 32];
    cursor.read_exact(&mut original_hash)?;
    let format = format_from_id(cursor.read_u8()?)?;
    let exact_mode = read_bool(&mut cursor)?;
    let fallback_used = read_bool(&mut cursor)?;
    let row_count = cursor.read_u64::<LittleEndian>()?;

    let schema_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut columns = Vec::with_capacity(schema_len);
    for _ in 0..schema_len {
        columns.push(read_column_schema(&mut cursor)?);
    }

    let plan_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut column_plans = Vec::with_capacity(plan_len);
    for _ in 0..plan_len {
        column_plans.push(ColumnPlan {
            name: read_string(&mut cursor)?,
            column_type: column_type_from_id(cursor.read_u8()?)?,
            codec: codec_from_id(cursor.read_u8()?)?,
            original_len: cursor.read_u64::<LittleEndian>()?,
            encoded_len_estimate: cursor.read_u64::<LittleEndian>()?,
        });
    }

    if cursor.position() != bytes.len() as u64 {
        return Err(anyhow!("trailing bytes in header"));
    }

    Ok(ArchiveHeader {
        version,
        header_schema_version,
        original_size,
        original_hash,
        format,
        exact_mode,
        fallback_used,
        schema: Schema { columns },
        row_count,
        column_plans,
    })
}

fn write_column_schema(out: &mut Vec<u8>, column: &ColumnSchema) -> anyhow::Result<()> {
    write_string(out, &column.name)?;
    out.write_u8(column_type_id(&column.column_type))?;
    out.write_u8(u8::from(column.nullable))?;
    Ok(())
}

fn read_column_schema(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<ColumnSchema> {
    Ok(ColumnSchema {
        name: read_string(cursor)?,
        column_type: column_type_from_id(cursor.read_u8()?)?,
        nullable: read_bool(cursor)?,
    })
}

fn write_string(out: &mut Vec<u8>, value: &str) -> anyhow::Result<()> {
    out.write_u32::<LittleEndian>(value.len() as u32)?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<String> {
    let len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut bytes = vec![0; len];
    cursor.read_exact(&mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

fn read_bool(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<bool> {
    match cursor.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(anyhow!("bad bool value {other}")),
    }
}

fn format_id(format: InputFormat) -> u8 {
    match format {
        InputFormat::Jsonl => 0,
        InputFormat::Csv => 1,
        InputFormat::Tsv => 2,
        InputFormat::Logs => 3,
        InputFormat::BinaryFallback => 4,
    }
}

fn format_from_id(id: u8) -> anyhow::Result<InputFormat> {
    match id {
        0 => Ok(InputFormat::Jsonl),
        1 => Ok(InputFormat::Csv),
        2 => Ok(InputFormat::Tsv),
        3 => Ok(InputFormat::Logs),
        4 => Ok(InputFormat::BinaryFallback),
        _ => Err(anyhow!("unknown input format id {id}")),
    }
}

fn column_type_id(column_type: &ColumnType) -> u8 {
    match column_type {
        ColumnType::Null => 0,
        ColumnType::Bool => 1,
        ColumnType::Integer => 2,
        ColumnType::Float => 3,
        ColumnType::String => 4,
        ColumnType::Enum => 5,
        ColumnType::Timestamp => 6,
        ColumnType::JsonRaw => 7,
    }
}

fn column_type_from_id(id: u8) -> anyhow::Result<ColumnType> {
    match id {
        0 => Ok(ColumnType::Null),
        1 => Ok(ColumnType::Bool),
        2 => Ok(ColumnType::Integer),
        3 => Ok(ColumnType::Float),
        4 => Ok(ColumnType::String),
        5 => Ok(ColumnType::Enum),
        6 => Ok(ColumnType::Timestamp),
        7 => Ok(ColumnType::JsonRaw),
        _ => Err(anyhow!("unknown column type id {id}")),
    }
}

fn codec_id(codec: ColumnCodec) -> u8 {
    match codec {
        ColumnCodec::Raw => 0,
        ColumnCodec::Dictionary => 1,
        ColumnCodec::Delta => 2,
        ColumnCodec::DeltaOfDelta => 3,
        ColumnCodec::Rle => 4,
        ColumnCodec::BitPack => 5,
        ColumnCodec::FrameOfReference => 6,
    }
}

fn codec_from_id(id: u8) -> anyhow::Result<ColumnCodec> {
    match id {
        0 => Ok(ColumnCodec::Raw),
        1 => Ok(ColumnCodec::Dictionary),
        2 => Ok(ColumnCodec::Delta),
        3 => Ok(ColumnCodec::DeltaOfDelta),
        4 => Ok(ColumnCodec::Rle),
        5 => Ok(ColumnCodec::BitPack),
        6 => Ok(ColumnCodec::FrameOfReference),
        _ => Err(anyhow!("unknown column codec id {id}")),
    }
}
