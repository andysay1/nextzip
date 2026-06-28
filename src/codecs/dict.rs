use std::collections::BTreeMap;

pub fn encode(values: &[String]) -> (Vec<String>, Vec<u32>) {
    let mut dict = Vec::new();
    let mut seen = BTreeMap::new();
    let mut indexes = Vec::with_capacity(values.len());
    for value in values {
        let idx = if let Some(idx) = seen.get(value) {
            *idx
        } else {
            let idx = dict.len() as u32;
            dict.push(value.clone());
            seen.insert(value.clone(), idx);
            idx
        };
        indexes.push(idx);
    }
    (dict, indexes)
}

pub fn decode(dict: &[String], indexes: &[u32]) -> Vec<String> {
    indexes
        .iter()
        .map(|idx| dict[*idx as usize].clone())
        .collect()
}
