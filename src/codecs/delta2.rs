pub fn encode(values: &[i64]) -> Vec<i64> {
    let delta = crate::codecs::delta::encode(values);
    crate::codecs::delta::encode(&delta)
}

pub fn decode(encoded: &[i64]) -> Vec<i64> {
    let delta = crate::codecs::delta::decode(encoded);
    crate::codecs::delta::decode(&delta)
}
