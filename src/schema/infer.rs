use std::collections::HashSet;

use crate::formats::table::Cell;
use crate::schema::ColumnType;

pub fn infer_type(values: &[Option<Cell>], row_count: usize) -> ColumnType {
    let present: Vec<&Cell> = values.iter().filter_map(|v| v.as_ref()).collect();
    if present.is_empty() {
        return ColumnType::Null;
    }
    if present.iter().all(|v| matches!(v, Cell::Bool(_))) {
        return ColumnType::Bool;
    }
    if present.iter().all(|v| matches!(v, Cell::Integer(_))) {
        return ColumnType::Integer;
    }
    if present
        .iter()
        .all(|v| matches!(v, Cell::Integer(_) | Cell::Float(_)))
    {
        return ColumnType::Float;
    }
    let unique = present
        .iter()
        .map(|v| v.to_stable_string())
        .collect::<HashSet<_>>()
        .len();
    if row_count > 0 && (unique as f64 / row_count as f64) <= 0.2 {
        ColumnType::Enum
    } else {
        ColumnType::String
    }
}
