use crate::formats::table::StoredTable;
use crate::header::InputFormat;

pub fn parse(input: &[u8]) -> anyhow::Result<StoredTable> {
    crate::formats::csv::parse_delimited(input, b'\t', InputFormat::Tsv)
}
