use std::collections::BTreeMap;

use crate::formats::table::{rows_to_table, Cell, LineEnding, StoredColumn, StoredTable};
use crate::header::InputFormat;

type ParsedTemplateLine = (String, String, Vec<(String, String)>);
const FIELD_ORDER_COLUMN: &str = "__nxz_field_order";

pub fn parse(input: &[u8]) -> anyhow::Result<StoredTable> {
    let text = std::str::from_utf8(input)?;
    let trailing_newline = text.ends_with('\n');
    let line_ending = if input.windows(2).any(|pair| pair == b"\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    };
    if let Some(table) = parse_template_logs(text, trailing_newline, line_ending) {
        return Ok(table);
    }
    let mut table = rows_to_table(
        InputFormat::Logs,
        text.lines()
            .map(|line| {
                let mut row = BTreeMap::new();
                for (idx, part) in line.split_whitespace().enumerate() {
                    row.insert(format!("f{idx}"), parse_log_cell(part));
                }
                row
            })
            .collect(),
        trailing_newline,
    );
    table.line_ending = line_ending;
    Ok(table)
}

pub fn reconstruct(table: &StoredTable) -> anyhow::Result<Vec<u8>> {
    if is_template_table(table) {
        return reconstruct_template_logs(table);
    }
    let mut out = String::new();
    for row_idx in 0..table.row_count {
        for (col_idx, col) in table.columns.iter().enumerate() {
            if col_idx > 0 {
                out.push(' ');
            }
            if let Some(cell) = col.values.get(row_idx).and_then(|v| v.as_ref()) {
                out.push_str(&cell.to_stable_string());
            }
        }
        push_line_ending(&mut out, table, row_idx);
    }
    Ok(out.into_bytes())
}

fn parse_template_logs(
    text: &str,
    trailing_newline: bool,
    line_ending: LineEnding,
) -> Option<StoredTable> {
    let parsed = text
        .lines()
        .map(parse_template_line)
        .collect::<Option<Vec<_>>>()?;
    if parsed.is_empty() {
        return None;
    }

    let mut names = vec![
        "timestamp".to_string(),
        "level".to_string(),
        FIELD_ORDER_COLUMN.to_string(),
    ];
    for (_, _, fields) in &parsed {
        for (key, _) in fields {
            if !names.iter().any(|name| name == key) {
                names.push(key.clone());
            }
        }
    }

    let mut columns = names
        .iter()
        .map(|name| StoredColumn {
            name: name.clone(),
            values: Vec::with_capacity(parsed.len()),
        })
        .collect::<Vec<_>>();

    for (timestamp, level, fields) in parsed {
        let field_order = fields
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        for column in &mut columns {
            let value = match column.name.as_str() {
                "timestamp" => Some(Cell::String(timestamp.clone())),
                "level" => Some(Cell::String(level.clone())),
                FIELD_ORDER_COLUMN => Some(Cell::String(field_order.clone())),
                key => fields.get(key).map(|value| parse_log_cell(value)),
            };
            column.values.push(value);
        }
    }

    Some(StoredTable {
        format: InputFormat::Logs,
        delimiter: None,
        line_ending,
        columns,
        row_count: text.lines().count(),
        trailing_newline,
    })
}

fn parse_template_line(line: &str) -> Option<ParsedTemplateLine> {
    let mut parts = line.split_whitespace();
    let timestamp = parts.next()?.to_string();
    let level = parts.next()?.to_string();
    if !matches!(
        level.as_str(),
        "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR"
    ) {
        return None;
    }
    let mut fields = Vec::new();
    for part in parts {
        let (key, value) = part.split_once('=')?;
        if key.is_empty() || value.is_empty() {
            return None;
        }
        fields.push((key.to_string(), value.to_string()));
    }
    if fields.is_empty() {
        return None;
    }
    Some((timestamp, level, fields))
}

fn is_template_table(table: &StoredTable) -> bool {
    table.format == InputFormat::Logs
        && table.columns.len() >= 3
        && table
            .columns
            .first()
            .is_some_and(|col| col.name == "timestamp")
        && table.columns.get(1).is_some_and(|col| col.name == "level")
}

fn reconstruct_template_logs(table: &StoredTable) -> anyhow::Result<Vec<u8>> {
    let mut out = String::new();
    let field_order_column = table
        .columns
        .iter()
        .find(|column| column.name == FIELD_ORDER_COLUMN);
    for row_idx in 0..table.row_count {
        write_template_cell(&mut out, table, "timestamp", row_idx);
        out.push(' ');
        write_template_cell(&mut out, table, "level", row_idx);

        let field_names = field_order_column
            .and_then(|column| column.values.get(row_idx))
            .and_then(|value| value.as_ref())
            .map(|cell| cell.to_stable_string())
            .unwrap_or_else(|| {
                table
                    .columns
                    .iter()
                    .filter(|column| column.name != "timestamp" && column.name != "level")
                    .filter(|column| column.name != FIELD_ORDER_COLUMN)
                    .filter(|column| column.values.get(row_idx).is_some_and(Option::is_some))
                    .map(|column| column.name.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            });
        for name in field_names.split(',').filter(|name| !name.is_empty()) {
            if let Some(column) = table.columns.iter().find(|column| column.name == name) {
                if let Some(cell) = column.values.get(row_idx).and_then(|v| v.as_ref()) {
                    out.push(' ');
                    out.push_str(name);
                    out.push('=');
                    out.push_str(&cell.to_stable_string());
                }
            }
        }
        push_line_ending(&mut out, table, row_idx);
    }
    Ok(out.into_bytes())
}

fn write_template_cell(out: &mut String, table: &StoredTable, name: &str, row_idx: usize) {
    if let Some(cell) = table
        .columns
        .iter()
        .find(|column| column.name == name)
        .and_then(|column| column.values.get(row_idx))
        .and_then(|value| value.as_ref())
    {
        out.push_str(&cell.to_stable_string());
    }
}

fn push_line_ending(out: &mut String, table: &StoredTable, row_idx: usize) {
    if row_idx + 1 < table.row_count || table.trailing_newline {
        match table.line_ending {
            LineEnding::Lf => out.push('\n'),
            LineEnding::CrLf => out.push_str("\r\n"),
        }
    }
}

fn parse_log_cell(value: &str) -> Cell {
    value
        .parse::<i64>()
        .map(Cell::Integer)
        .unwrap_or_else(|_| Cell::String(value.to_string()))
}
