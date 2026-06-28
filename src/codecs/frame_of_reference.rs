pub fn encode(values: &[i64]) -> Option<(i64, Vec<u64>)> {
    let base = *values.iter().min()?;
    Some((base, values.iter().map(|v| (*v - base) as u64).collect()))
}

pub fn decode(base: i64, offsets: &[u64]) -> Vec<i64> {
    offsets.iter().map(|v| base + *v as i64).collect()
}
