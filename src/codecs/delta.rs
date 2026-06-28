pub fn encode(values: &[i64]) -> Vec<i64> {
    if values.is_empty() {
        return vec![];
    }
    let mut out = Vec::with_capacity(values.len());
    out.push(values[0]);
    out.extend(values.windows(2).map(|w| w[1] - w[0]));
    out
}

pub fn decode(encoded: &[i64]) -> Vec<i64> {
    if encoded.is_empty() {
        return vec![];
    }
    let mut out = Vec::with_capacity(encoded.len());
    let mut cur = encoded[0];
    out.push(cur);
    for delta in &encoded[1..] {
        cur += delta;
        out.push(cur);
    }
    out
}
