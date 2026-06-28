use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use crate::column::payload::estimate_encoded_len;
use crate::column::planner::choose_codec;
use crate::column::ColumnPlan;
use crate::header::InputFormat;
use crate::schema::infer::infer_type;
use crate::schema::{ColumnSchema, Schema};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cell {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl Cell {
    pub fn to_stable_string(&self) -> String {
        match self {
            Cell::Null => "null".to_string(),
            Cell::Bool(v) => v.to_string(),
            Cell::Integer(v) => v.to_string(),
            Cell::Float(v) => v.to_string(),
            Cell::String(v) => v.clone(),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Cell::Null => Value::Null,
            Cell::Bool(v) => Value::Bool(*v),
            Cell::Integer(v) => Value::Number(Number::from(*v)),
            Cell::Float(v) => Number::from_f64(*v)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Cell::String(v) => Value::String(v.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredColumn {
    pub name: String,
    pub values: Vec<Option<Cell>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTable {
    pub format: InputFormat,
    pub delimiter: Option<u8>,
    pub line_ending: LineEnding,
    pub columns: Vec<StoredColumn>,
    pub row_count: usize,
    pub trailing_newline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl StoredTable {
    pub fn schema_and_plans(&self) -> (Schema, Vec<ColumnPlan>) {
        let mut schema = Vec::new();
        let mut plans = Vec::new();
        for col in &self.columns {
            let column_type = infer_type(&col.values, self.row_count);
            let codec = choose_codec(&col.values);
            let original_len = bincode::serialized_size(&col.values).unwrap_or(0);
            let encoded_len_estimate = estimate_encoded_len(&col.values, codec);
            schema.push(ColumnSchema {
                name: col.name.clone(),
                column_type: column_type.clone(),
                nullable: col.values.iter().any(|v| v.is_none()),
            });
            plans.push(ColumnPlan {
                name: col.name.clone(),
                column_type,
                codec,
                original_len,
                encoded_len_estimate,
            });
        }
        (Schema { columns: schema }, plans)
    }
}

pub fn rows_to_table(
    format: InputFormat,
    rows: Vec<BTreeMap<String, Cell>>,
    trailing_newline: bool,
) -> StoredTable {
    let names: BTreeSet<String> = rows.iter().flat_map(|row| row.keys().cloned()).collect();
    let columns = names
        .into_iter()
        .map(|name| StoredColumn {
            values: rows.iter().map(|row| row.get(&name).cloned()).collect(),
            name,
        })
        .collect();
    StoredTable {
        format,
        delimiter: None,
        line_ending: LineEnding::Lf,
        columns,
        row_count: rows.len(),
        trailing_newline,
    }
}

pub fn value_to_cell(value: &Value) -> Cell {
    match value {
        Value::Null => Cell::Null,
        Value::Bool(v) => Cell::Bool(*v),
        Value::Number(n) => n
            .as_i64()
            .map(Cell::Integer)
            .or_else(|| n.as_f64().map(Cell::Float))
            .unwrap_or(Cell::Null),
        Value::String(v) => Cell::String(v.clone()),
        other => Cell::String(other.to_string()),
    }
}
