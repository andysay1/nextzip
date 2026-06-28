use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::column::ColumnCodec;
use crate::formats::table::{Cell, LineEnding, StoredColumn, StoredTable};
use crate::header::InputFormat;

const TARGET_BLOCK_ROWS: usize = 16_384;

#[derive(Debug, Clone)]
pub struct CodecStat {
    pub column_id: usize,
    pub column_name: String,
    pub codec: ColumnCodec,
    pub chunks: u64,
    pub bytes: u64,
}

pub fn encode_table(
    table: &StoredTable,
    _plans: &[crate::column::ColumnPlan],
) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    write_meta(&mut out, table)?;

    let block_count = table.row_count.div_ceil(TARGET_BLOCK_ROWS).max(1);
    out.write_u32::<LittleEndian>(block_count as u32)?;
    for block_idx in 0..block_count {
        let start = block_idx * TARGET_BLOCK_ROWS;
        let end = ((block_idx + 1) * TARGET_BLOCK_ROWS).min(table.row_count);
        out.write_u32::<LittleEndian>((end - start) as u32)?;

        for (column_id, column) in table.columns.iter().enumerate() {
            let values = &column.values[start..end];
            let codec = choose_best_codec(values);
            let chunk = encode_column_chunk(values, codec)?;
            out.write_u32::<LittleEndian>(column_id as u32)?;
            out.write_u8(codec_id(codec))?;
            out.write_u64::<LittleEndian>(chunk.len() as u64)?;
            out.extend_from_slice(&chunk);
        }
    }
    Ok(out)
}

pub fn decode_table(bytes: &[u8]) -> anyhow::Result<StoredTable> {
    let mut cursor = Cursor::new(bytes);
    let mut table = read_meta(&mut cursor)?;
    let block_count = cursor.read_u32::<LittleEndian>()? as usize;

    let mut columns = table
        .columns
        .iter()
        .map(|column| StoredColumn {
            name: column.name.clone(),
            values: Vec::with_capacity(table.row_count),
        })
        .collect::<Vec<_>>();

    for _ in 0..block_count {
        let block_rows = cursor.read_u32::<LittleEndian>()? as usize;
        for _ in 0..columns.len() {
            let column_id = cursor.read_u32::<LittleEndian>()? as usize;
            let codec = codec_from_id(cursor.read_u8()?)?;
            let chunk_len = cursor.read_u64::<LittleEndian>()? as usize;
            let mut chunk = vec![0; chunk_len];
            cursor.read_exact(&mut chunk)?;
            let values = decode_column_chunk(&chunk, codec, block_rows)?;
            columns
                .get_mut(column_id)
                .with_context(|| format!("bad column id {column_id}"))?
                .values
                .extend(values);
        }
    }

    table.columns = columns;
    Ok(table)
}

pub fn block_count(bytes: &[u8]) -> anyhow::Result<usize> {
    let mut cursor = Cursor::new(bytes);
    let _table = read_meta(&mut cursor)?;
    Ok(cursor.read_u32::<LittleEndian>()? as usize)
}

pub fn codec_stats(bytes: &[u8]) -> anyhow::Result<Vec<CodecStat>> {
    let mut cursor = Cursor::new(bytes);
    let table = read_meta(&mut cursor)?;
    let block_count = cursor.read_u32::<LittleEndian>()? as usize;
    let mut stats = BTreeMap::<(usize, ColumnCodec), (u64, u64)>::new();

    for _ in 0..block_count {
        let _block_rows = cursor.read_u32::<LittleEndian>()? as usize;
        for _ in 0..table.columns.len() {
            let column_id = cursor.read_u32::<LittleEndian>()? as usize;
            let codec = codec_from_id(cursor.read_u8()?)?;
            let chunk_len = cursor.read_u64::<LittleEndian>()? as usize;
            let current = stats.entry((column_id, codec)).or_default();
            current.0 += 1;
            current.1 += chunk_len as u64;
            cursor.set_position(cursor.position() + chunk_len as u64);
            if cursor.position() > bytes.len() as u64 {
                return Err(anyhow!("column chunk exceeds payload length"));
            }
        }
    }

    Ok(stats
        .into_iter()
        .map(|((column_id, codec), (chunks, bytes))| CodecStat {
            column_name: table
                .columns
                .get(column_id)
                .map(|column| column.name.clone())
                .unwrap_or_else(|| format!("col{column_id}")),
            column_id,
            codec,
            chunks,
            bytes,
        })
        .collect())
}

pub fn estimate_encoded_len(values: &[Option<Cell>], codec: ColumnCodec) -> u64 {
    encode_column_chunk(values, codec)
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(u64::MAX)
}

pub fn choose_best_codec(values: &[Option<Cell>]) -> ColumnCodec {
    let all_ints = values
        .iter()
        .all(|value| matches!(value, None | Some(Cell::Integer(_))));
    let all_strings = values
        .iter()
        .all(|value| matches!(value, None | Some(Cell::String(_))));
    let candidates: &[ColumnCodec] = if all_ints {
        if is_near_linear(values) {
            let delta = compressed_chunk_size(values, ColumnCodec::Delta);
            let delta2 = compressed_chunk_size(values, ColumnCodec::DeltaOfDelta);
            if delta2 <= delta + 16 {
                return ColumnCodec::DeltaOfDelta;
            }
        }
        &[
            ColumnCodec::Raw,
            ColumnCodec::DeltaOfDelta,
            ColumnCodec::Delta,
            ColumnCodec::BitPack,
            ColumnCodec::FrameOfReference,
        ]
    } else if all_strings {
        &[ColumnCodec::Raw, ColumnCodec::Dictionary, ColumnCodec::Rle]
    } else {
        &[ColumnCodec::Raw, ColumnCodec::Rle]
    };

    candidates
        .iter()
        .copied()
        .min_by_key(|codec| compressed_chunk_size(values, *codec))
        .unwrap_or(ColumnCodec::Raw)
}

fn is_near_linear(values: &[Option<Cell>]) -> bool {
    let ints = values
        .iter()
        .filter_map(|value| match value {
            Some(Cell::Integer(value)) => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();
    if ints.len() < 4 {
        return false;
    }
    let deltas = ints
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    let stable = deltas
        .windows(2)
        .filter(|pair| pair[1] - pair[0] == 0)
        .count();
    stable * 100 >= deltas.len().saturating_sub(1) * 90
}

fn compressed_chunk_size(values: &[Option<Cell>], codec: ColumnCodec) -> usize {
    let bytes = encode_column_chunk(values, codec).unwrap_or_default();
    zstd::stream::encode_all(bytes.as_slice(), 1)
        .map(|compressed| compressed.len())
        .unwrap_or(bytes.len())
}

fn write_meta(out: &mut Vec<u8>, table: &StoredTable) -> anyhow::Result<()> {
    out.write_u8(format_id(table.format))?;
    match table.delimiter {
        Some(delimiter) => {
            out.write_u8(1)?;
            out.write_u8(delimiter)?;
        }
        None => out.write_u8(0)?,
    }
    out.write_u8(u8::from(table.trailing_newline))?;
    out.write_u8(match table.line_ending {
        LineEnding::Lf => 0,
        LineEnding::CrLf => 1,
    })?;
    out.write_u64::<LittleEndian>(table.row_count as u64)?;
    out.write_u32::<LittleEndian>(table.columns.len() as u32)?;
    for column in &table.columns {
        write_string(out, &column.name)?;
    }
    Ok(())
}

fn read_meta(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<StoredTable> {
    let format = format_from_id(cursor.read_u8()?)?;
    let delimiter = match cursor.read_u8()? {
        0 => None,
        1 => Some(cursor.read_u8()?),
        other => return Err(anyhow!("bad delimiter marker {other}")),
    };
    let trailing_newline = cursor.read_u8()? != 0;
    let line_ending = match cursor.read_u8()? {
        0 => LineEnding::Lf,
        1 => LineEnding::CrLf,
        other => return Err(anyhow!("bad line ending id {other}")),
    };
    let row_count = cursor.read_u64::<LittleEndian>()? as usize;
    let column_count = cursor.read_u32::<LittleEndian>()? as usize;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        columns.push(StoredColumn {
            name: read_string(cursor)?,
            values: Vec::new(),
        });
    }
    Ok(StoredTable {
        format,
        delimiter,
        line_ending,
        columns,
        row_count,
        trailing_newline,
    })
}

fn encode_column_chunk(values: &[Option<Cell>], codec: ColumnCodec) -> anyhow::Result<Vec<u8>> {
    match codec {
        ColumnCodec::Dictionary => encode_string_dictionary(values),
        ColumnCodec::Delta => encode_delta(values),
        ColumnCodec::DeltaOfDelta => encode_delta2(values),
        ColumnCodec::BitPack => encode_bitpack(values),
        ColumnCodec::FrameOfReference => encode_frame_of_reference(values),
        ColumnCodec::Rle => encode_rle(values),
        ColumnCodec::Raw => encode_raw(values),
    }
}

fn decode_column_chunk(
    chunk: &[u8],
    codec: ColumnCodec,
    row_count: usize,
) -> anyhow::Result<Vec<Option<Cell>>> {
    let mut cursor = Cursor::new(chunk);
    match codec {
        ColumnCodec::Raw => decode_raw(&mut cursor, row_count),
        ColumnCodec::Dictionary => decode_string_dictionary(&mut cursor, row_count),
        ColumnCodec::Delta => decode_delta(&mut cursor, row_count),
        ColumnCodec::DeltaOfDelta => decode_delta2(&mut cursor, row_count),
        ColumnCodec::BitPack => decode_bitpack(&mut cursor, row_count),
        ColumnCodec::FrameOfReference => decode_frame_of_reference(&mut cursor, row_count),
        ColumnCodec::Rle => decode_rle(&mut cursor, row_count),
    }
}

fn encode_raw(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    for value in values {
        match value {
            None => out.write_u8(0)?,
            Some(cell) => {
                out.write_u8(1)?;
                write_cell(&mut out, cell)?;
            }
        }
    }
    Ok(out)
}

fn decode_raw(cursor: &mut Cursor<&[u8]>, row_count: usize) -> anyhow::Result<Vec<Option<Cell>>> {
    let mut out = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        out.push(match cursor.read_u8()? {
            0 => None,
            1 => Some(read_cell(cursor)?),
            other => return Err(anyhow!("bad raw presence marker {other}")),
        });
    }
    Ok(out)
}

fn encode_string_dictionary(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let present = presence_bitmap(values);
    let strings = values
        .iter()
        .filter_map(|value| match value {
            Some(Cell::String(value)) => Some(value.as_str()),
            None => None,
            _ => None,
        })
        .collect::<Vec<_>>();
    if strings.len() != values.iter().filter(|value| value.is_some()).count() {
        return encode_raw(values);
    }

    let mut dict = Vec::<String>::new();
    let mut seen = std::collections::BTreeMap::<&str, u64>::new();
    let mut indexes = Vec::with_capacity(strings.len());
    for value in strings {
        let idx = if let Some(idx) = seen.get(value) {
            *idx
        } else {
            let idx = dict.len() as u64;
            dict.push(value.to_string());
            seen.insert(value, idx);
            idx
        };
        indexes.push(idx);
    }

    let bits = crate::codecs::bitpack::bits_required(dict.len().saturating_sub(1) as u64);
    let packed = crate::codecs::bitpack::encode(&indexes, bits);
    let mut out = Vec::new();
    write_bytes(&mut out, &present)?;
    out.write_u32::<LittleEndian>(dict.len() as u32)?;
    for value in dict {
        write_string(&mut out, &value)?;
    }
    out.write_u8(bits)?;
    out.write_u64::<LittleEndian>(indexes.len() as u64)?;
    write_bytes(&mut out, &packed)?;
    Ok(out)
}

fn decode_string_dictionary(
    cursor: &mut Cursor<&[u8]>,
    row_count: usize,
) -> anyhow::Result<Vec<Option<Cell>>> {
    let present = read_bytes(cursor)?;
    let dict_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut dict = Vec::with_capacity(dict_len);
    for _ in 0..dict_len {
        dict.push(read_string(cursor)?);
    }
    let bits = cursor.read_u8()?;
    let value_count = cursor.read_u64::<LittleEndian>()? as usize;
    let packed = read_bytes(cursor)?;
    let mut indexes = crate::codecs::bitpack::decode(&packed, bits, value_count).into_iter();
    Ok(unpack_presence(&present, row_count)
        .into_iter()
        .map(|has_value| {
            if has_value {
                indexes
                    .next()
                    .and_then(|idx| dict.get(idx as usize).cloned())
                    .map(Cell::String)
            } else {
                None
            }
        })
        .collect())
}

fn encode_delta(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let present = presence_bitmap(values);
    let ints = integer_values(values)?;
    let encoded = crate::codecs::delta::encode(&ints);
    let mut out = Vec::new();
    write_bytes(&mut out, &present)?;
    out.write_u64::<LittleEndian>(encoded.len() as u64)?;
    for value in encoded {
        write_varint(&mut out, crate::codecs::zigzag::zigzag_i64(value));
    }
    Ok(out)
}

fn decode_delta(cursor: &mut Cursor<&[u8]>, row_count: usize) -> anyhow::Result<Vec<Option<Cell>>> {
    let present = read_bytes(cursor)?;
    let count = cursor.read_u64::<LittleEndian>()? as usize;
    let mut encoded = Vec::with_capacity(count);
    for _ in 0..count {
        encoded.push(crate::codecs::zigzag::unzigzag_u64(read_varint(cursor)?));
    }
    let mut decoded = crate::codecs::delta::decode(&encoded).into_iter();
    Ok(unpack_presence(&present, row_count)
        .into_iter()
        .map(|has_value| {
            if has_value {
                decoded.next().map(Cell::Integer)
            } else {
                None
            }
        })
        .collect())
}

fn encode_delta2(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let present = presence_bitmap(values);
    let ints = integer_values(values)?;
    let mut out = Vec::new();
    write_bytes(&mut out, &present)?;
    out.write_u64::<LittleEndian>(ints.len() as u64)?;
    if let Some(base) = ints.first() {
        write_varint(&mut out, crate::codecs::zigzag::zigzag_i64(*base));
    }
    if ints.len() >= 2 {
        let first_delta = ints[1] - ints[0];
        write_varint(&mut out, crate::codecs::zigzag::zigzag_i64(first_delta));
        let mut prev_delta = first_delta;
        for pair in ints[1..].windows(2) {
            let delta = pair[1] - pair[0];
            write_varint(
                &mut out,
                crate::codecs::zigzag::zigzag_i64(delta - prev_delta),
            );
            prev_delta = delta;
        }
    }
    Ok(out)
}

fn decode_delta2(
    cursor: &mut Cursor<&[u8]>,
    row_count: usize,
) -> anyhow::Result<Vec<Option<Cell>>> {
    let present = read_bytes(cursor)?;
    let count = cursor.read_u64::<LittleEndian>()? as usize;
    let mut values = Vec::with_capacity(count);
    if count > 0 {
        let mut current = crate::codecs::zigzag::unzigzag_u64(read_varint(cursor)?);
        values.push(current);
        if count > 1 {
            let mut delta = crate::codecs::zigzag::unzigzag_u64(read_varint(cursor)?);
            current += delta;
            values.push(current);
            for _ in 2..count {
                delta += crate::codecs::zigzag::unzigzag_u64(read_varint(cursor)?);
                current += delta;
                values.push(current);
            }
        }
    }
    let mut decoded = values.into_iter();
    Ok(unpack_presence(&present, row_count)
        .into_iter()
        .map(|has_value| {
            if has_value {
                decoded.next().map(Cell::Integer)
            } else {
                None
            }
        })
        .collect())
}

fn encode_bitpack(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let present = presence_bitmap(values);
    let ints = integer_values(values)?;
    if ints.iter().any(|value| *value < 0) {
        return encode_delta(values);
    }
    let unsigned = ints.iter().map(|value| *value as u64).collect::<Vec<_>>();
    let bits = crate::codecs::bitpack::bits_required(unsigned.iter().max().copied().unwrap_or(0));
    let packed = crate::codecs::bitpack::encode(&unsigned, bits);
    let mut out = Vec::new();
    write_bytes(&mut out, &present)?;
    out.write_u8(bits)?;
    out.write_u64::<LittleEndian>(unsigned.len() as u64)?;
    write_bytes(&mut out, &packed)?;
    Ok(out)
}

fn decode_bitpack(
    cursor: &mut Cursor<&[u8]>,
    row_count: usize,
) -> anyhow::Result<Vec<Option<Cell>>> {
    let present = read_bytes(cursor)?;
    let bits = cursor.read_u8()?;
    let value_count = cursor.read_u64::<LittleEndian>()? as usize;
    let packed = read_bytes(cursor)?;
    let mut decoded = crate::codecs::bitpack::decode(&packed, bits, value_count).into_iter();
    Ok(unpack_presence(&present, row_count)
        .into_iter()
        .map(|has_value| {
            if has_value {
                decoded.next().map(|value| Cell::Integer(value as i64))
            } else {
                None
            }
        })
        .collect())
}

fn encode_frame_of_reference(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let present = presence_bitmap(values);
    let ints = integer_values(values)?;
    let base = *ints.iter().min().unwrap_or(&0);
    let offsets = ints
        .iter()
        .map(|value| (*value - base).try_into())
        .collect::<Result<Vec<u64>, _>>()?;
    let bits = crate::codecs::bitpack::bits_required(offsets.iter().max().copied().unwrap_or(0));
    let packed = crate::codecs::bitpack::encode(&offsets, bits);
    let mut out = Vec::new();
    write_bytes(&mut out, &present)?;
    out.write_i64::<LittleEndian>(base)?;
    out.write_u8(bits)?;
    out.write_u64::<LittleEndian>(offsets.len() as u64)?;
    write_bytes(&mut out, &packed)?;
    Ok(out)
}

fn decode_frame_of_reference(
    cursor: &mut Cursor<&[u8]>,
    row_count: usize,
) -> anyhow::Result<Vec<Option<Cell>>> {
    let present = read_bytes(cursor)?;
    let base = cursor.read_i64::<LittleEndian>()?;
    let bits = cursor.read_u8()?;
    let value_count = cursor.read_u64::<LittleEndian>()? as usize;
    let packed = read_bytes(cursor)?;
    let mut decoded = crate::codecs::bitpack::decode(&packed, bits, value_count).into_iter();
    Ok(unpack_presence(&present, row_count)
        .into_iter()
        .map(|has_value| {
            if has_value {
                decoded
                    .next()
                    .map(|offset| Cell::Integer(base + offset as i64))
            } else {
                None
            }
        })
        .collect())
}

fn encode_rle(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut runs = Vec::<(Option<Cell>, u64)>::new();
    for value in values {
        if let Some((last, count)) = runs.last_mut() {
            if last == value {
                *count += 1;
                continue;
            }
        }
        runs.push((value.clone(), 1));
    }
    out.write_u64::<LittleEndian>(runs.len() as u64)?;
    for (value, count) in runs {
        out.write_u64::<LittleEndian>(count)?;
        match value {
            None => out.write_u8(0)?,
            Some(value) => {
                out.write_u8(1)?;
                write_cell(&mut out, &value)?;
            }
        }
    }
    Ok(out)
}

fn decode_rle(cursor: &mut Cursor<&[u8]>, row_count: usize) -> anyhow::Result<Vec<Option<Cell>>> {
    let run_count = cursor.read_u64::<LittleEndian>()? as usize;
    let mut out = Vec::with_capacity(row_count);
    for _ in 0..run_count {
        let count = cursor.read_u64::<LittleEndian>()? as usize;
        let value = match cursor.read_u8()? {
            0 => None,
            1 => Some(read_cell(cursor)?),
            other => return Err(anyhow!("bad rle marker {other}")),
        };
        out.extend(std::iter::repeat_n(value, count));
    }
    if out.len() != row_count {
        return Err(anyhow!(
            "rle decoded {} rows, expected {row_count}",
            out.len()
        ));
    }
    Ok(out)
}

fn integer_values(values: &[Option<Cell>]) -> anyhow::Result<Vec<i64>> {
    values
        .iter()
        .filter_map(|value| match value {
            Some(Cell::Integer(value)) => Some(Ok(*value)),
            None => None,
            _ => Some(Err(anyhow!("non-integer value in integer chunk"))),
        })
        .collect()
}

fn presence_bitmap(values: &[Option<Cell>]) -> Vec<u8> {
    let bools = values.iter().map(Option::is_some).collect::<Vec<_>>();
    pack_presence(&bools)
}

fn pack_presence(values: &[bool]) -> Vec<u8> {
    let mut out = vec![0u8; values.len().div_ceil(8)];
    for (idx, value) in values.iter().enumerate() {
        if *value {
            out[idx / 8] |= 1 << (idx % 8);
        }
    }
    out
}

fn unpack_presence(bytes: &[u8], count: usize) -> Vec<bool> {
    (0..count)
        .map(|idx| {
            bytes
                .get(idx / 8)
                .is_some_and(|byte| byte & (1 << (idx % 8)) != 0)
        })
        .collect()
}

fn write_cell(out: &mut Vec<u8>, cell: &Cell) -> anyhow::Result<()> {
    match cell {
        Cell::Null => out.write_u8(0)?,
        Cell::Bool(value) => {
            out.write_u8(1)?;
            out.write_u8(u8::from(*value))?;
        }
        Cell::Integer(value) => {
            out.write_u8(2)?;
            out.write_i64::<LittleEndian>(*value)?;
        }
        Cell::Float(value) => {
            out.write_u8(3)?;
            out.write_f64::<LittleEndian>(*value)?;
        }
        Cell::String(value) => {
            out.write_u8(4)?;
            write_string(out, value)?;
        }
    }
    Ok(())
}

fn read_cell(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Cell> {
    match cursor.read_u8()? {
        0 => Ok(Cell::Null),
        1 => Ok(Cell::Bool(cursor.read_u8()? != 0)),
        2 => Ok(Cell::Integer(cursor.read_i64::<LittleEndian>()?)),
        3 => Ok(Cell::Float(cursor.read_f64::<LittleEndian>()?)),
        4 => Ok(Cell::String(read_string(cursor)?)),
        other => Err(anyhow!("bad cell tag {other}")),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> anyhow::Result<()> {
    write_bytes(out, value.as_bytes())
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<String> {
    Ok(String::from_utf8(read_bytes(cursor)?)?)
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> anyhow::Result<()> {
    out.write_u64::<LittleEndian>(bytes.len() as u64)?;
    out.extend_from_slice(bytes);
    Ok(())
}

fn read_bytes(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Vec<u8>> {
    let len = cursor.read_u64::<LittleEndian>()? as usize;
    let mut bytes = vec![0; len];
    cursor.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<u64> {
    let mut out = 0u64;
    let mut shift = 0;
    loop {
        let byte = cursor.read_u8()?;
        out |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(out);
        }
        shift += 7;
        if shift >= 64 {
            return Err(anyhow!("varint too long"));
        }
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
        _ => Err(anyhow!("unknown codec id {id}")),
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
        _ => Err(anyhow!("unknown format id {id}")),
    }
}
