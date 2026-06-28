use crate::formats::table::Cell;

pub fn encode_raw(values: &[Option<Cell>]) -> anyhow::Result<Vec<u8>> {
    Ok(bincode::serialize(values)?)
}
