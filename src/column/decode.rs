use crate::formats::table::Cell;

pub fn decode_raw(bytes: &[u8]) -> anyhow::Result<Vec<Option<Cell>>> {
    Ok(bincode::deserialize(bytes)?)
}
