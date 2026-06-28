use std::collections::HashSet;

use crate::formats::table::Cell;

pub fn unique_count(values: &[Option<Cell>]) -> usize {
    values
        .iter()
        .filter_map(|v| v.as_ref())
        .map(|v| v.to_stable_string())
        .collect::<HashSet<_>>()
        .len()
}
