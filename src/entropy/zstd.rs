pub fn encode(input: &[u8], level: i32) -> anyhow::Result<Vec<u8>> {
    Ok(zstd::stream::encode_all(input, level)?)
}

pub fn decode(input: &[u8]) -> anyhow::Result<Vec<u8>> {
    Ok(zstd::stream::decode_all(input)?)
}
