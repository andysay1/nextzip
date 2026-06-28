use std::collections::BTreeMap;

use anyhow::{anyhow, Context};
use serde_json::{Map, Value};

use crate::formats::table::{
    rows_to_table, value_to_cell, Cell, LineEnding, StoredColumn, StoredTable,
};
use crate::header::InputFormat;
use crate::schema::flatten::flatten_json;

pub fn parse(input: &[u8]) -> anyhow::Result<StoredTable> {
    let text = std::str::from_utf8(input)?;
    let trailing_newline = text.ends_with('\n');
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value =
            serde_json::from_str(line).with_context(|| format!("invalid jsonl line: {line}"))?;
        if !value.is_object() {
            return Err(anyhow!("jsonl row is not an object"));
        }
        let row = flatten_json(&value)
            .into_iter()
            .map(|(k, v)| (k, value_to_cell(&v)))
            .collect::<BTreeMap<_, _>>();
        rows.push(row);
    }
    Ok(rows_to_table(InputFormat::Jsonl, rows, trailing_newline))
}

pub fn parse_exact(input: &[u8]) -> anyhow::Result<StoredTable> {
    let text = std::str::from_utf8(input)?;
    let trailing_newline = text.ends_with('\n');
    let line_ending = if input.windows(2).any(|pair| pair == b"\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    };
    let values = text
        .lines()
        .map(|line| Some(Cell::String(line.to_string())))
        .collect::<Vec<_>>();
    Ok(StoredTable {
        format: InputFormat::Jsonl,
        delimiter: None,
        line_ending,
        columns: vec![StoredColumn {
            name: "__nxz_raw_line".to_string(),
            values,
        }],
        row_count: text.lines().count(),
        trailing_newline,
    })
}

pub fn reconstruct(table: &StoredTable) -> anyhow::Result<Vec<u8>> {
    if let Some(raw_lines) = table
        .columns
        .iter()
        .find(|column| column.name == "__nxz_raw_line")
    {
        return reconstruct_exact(raw_lines, table);
    }
    let mut out = Vec::new();
    for row_idx in 0..table.row_count {
        let mut object = Map::new();
        for col in &table.columns {
            if let Some(cell) = col.values.get(row_idx).and_then(|v| v.as_ref()) {
                insert_path(&mut object, &col.name, cell);
            }
        }
        serde_json::to_writer(&mut out, &Value::Object(object))?;
        if row_idx + 1 < table.row_count || table.trailing_newline {
            out.push(b'\n');
        }
    }
    Ok(out)
}

fn reconstruct_exact(raw_lines: &StoredColumn, table: &StoredTable) -> anyhow::Result<Vec<u8>> {
    let mut out = String::new();
    for row_idx in 0..table.row_count {
        if let Some(Cell::String(line)) = raw_lines
            .values
            .get(row_idx)
            .and_then(|value| value.as_ref())
        {
            out.push_str(line);
        }
        if row_idx + 1 < table.row_count || table.trailing_newline {
            match table.line_ending {
                LineEnding::Lf => out.push('\n'),
                LineEnding::CrLf => out.push_str("\r\n"),
            }
        }
    }
    Ok(out.into_bytes())
}

fn insert_path(object: &mut Map<String, Value>, path: &str, cell: &Cell) {
    let mut parts = path.split('.').peekable();
    let mut current = object;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            current.insert(part.to_string(), cell.to_json());
            return;
        }
        let value = current
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !value.is_object() {
            *value = Value::Object(Map::new());
        }
        current = value.as_object_mut().expect("object just inserted");
    }
}
