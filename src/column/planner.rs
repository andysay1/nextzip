use crate::column::ColumnCodec;
use crate::formats::table::Cell;

pub fn choose_codec(values: &[Option<Cell>]) -> ColumnCodec {
    crate::column::payload::choose_best_codec(values)
}
