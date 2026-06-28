use anyhow::Context;

use crate::formats::table::{Cell, LineEnding, StoredColumn, StoredTable};
use crate::header::InputFormat;

pub fn parse(input: &[u8]) -> anyhow::Result<StoredTable> {
    parse_delimited(input, b',', InputFormat::Csv)
}

pub fn parse_delimited(
    input: &[u8],
    delimiter: u8,
    format: InputFormat,
) -> anyhow::Result<StoredTable> {
    let trailing_newline = input.ends_with(b"\n");
    let line_ending = if input.windows(2).any(|pair| pair == b"\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_reader(input);
    let headers: Vec<String> = rdr.headers()?.iter().map(|s| s.to_string()).collect();
    let mut columns = headers
        .iter()
        .map(|name| StoredColumn {
            name: name.clone(),
            values: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut row_count = 0usize;
    for record in rdr.records() {
        let record = record?;
        if record.len() > columns.len() {
            for idx in columns.len()..record.len() {
                columns.push(StoredColumn {
                    name: format!("col{idx}"),
                    values: vec![None; row_count],
                });
            }
        }
        for (idx, column) in columns.iter_mut().enumerate() {
            column.values.push(record.get(idx).map(parse_cell));
        }
        row_count += 1;
    }
    Ok(StoredTable {
        format,
        delimiter: Some(delimiter),
        line_ending,
        columns,
        row_count,
        trailing_newline,
    })
}

pub fn reconstruct(table: &StoredTable) -> anyhow::Result<Vec<u8>> {
    let delimiter = table.delimiter.context("missing delimiter")?;
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .terminator(match table.line_ending {
            LineEnding::Lf => csv::Terminator::Any(b'\n'),
            LineEnding::CrLf => csv::Terminator::CRLF,
        })
        .from_writer(vec![]);
    let headers: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    wtr.write_record(&headers)?;
    for row_idx in 0..table.row_count {
        let row: Vec<String> = table
            .columns
            .iter()
            .map(|col| {
                col.values
                    .get(row_idx)
                    .and_then(|v| v.as_ref())
                    .map(|v| v.to_stable_string())
                    .unwrap_or_default()
            })
            .collect();
        wtr.write_record(row)?;
    }
    let mut out = wtr.into_inner()?;
    if !table.trailing_newline {
        match table.line_ending {
            LineEnding::Lf if out.ends_with(b"\n") => {
                out.pop();
            }
            LineEnding::CrLf if out.ends_with(b"\r\n") => {
                out.truncate(out.len() - 2);
            }
            _ => {}
        }
    }
    Ok(out)
}

fn parse_cell(value: &str) -> Cell {
    if let Ok(v) = value.parse::<i64>() {
        Cell::Integer(v)
    } else if let Ok(v) = value.parse::<f64>() {
        Cell::Float(v)
    } else if let Ok(v) = value.parse::<bool>() {
        Cell::Bool(v)
    } else {
        Cell::String(value.to_string())
    }
}
