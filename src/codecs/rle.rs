pub fn encode<T: Clone + PartialEq>(values: &[T]) -> Vec<(T, u64)> {
    let mut out = Vec::new();
    for value in values {
        if let Some((last, count)) = out.last_mut() {
            if *last == *value {
                *count += 1;
                continue;
            }
        }
        out.push((value.clone(), 1));
    }
    out
}

pub fn decode<T: Clone>(encoded: &[(T, u64)]) -> Vec<T> {
    encoded
        .iter()
        .flat_map(|(value, count)| std::iter::repeat_n(value.clone(), *count as usize))
        .collect()
}
