pub fn encode<T: serde::Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    Ok(bincode::serialize(value)?)
}

pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> anyhow::Result<T> {
    Ok(bincode::deserialize(bytes)?)
}
