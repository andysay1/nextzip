use serde_json::Value;

use crate::header::InputFormat;

pub fn detect_format(sample: &[u8]) -> InputFormat {
    if sample.iter().take(4096).any(|b| *b == 0) {
        return InputFormat::BinaryFallback;
    }
    let text = match std::str::from_utf8(sample) {
        Ok(text) => text,
        Err(_) => return InputFormat::BinaryFallback,
    };
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(200)
        .collect();
    if lines.is_empty() {
        return InputFormat::BinaryFallback;
    }
    let json_objects = lines
        .iter()
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .is_some_and(|v| v.is_object())
        })
        .count();
    if lines.len() >= 20 && json_objects * 100 >= lines.len() * 90 {
        return InputFormat::Jsonl;
    }

    for (delim, format) in [
        (',', InputFormat::Csv),
        (';', InputFormat::Csv),
        ('\t', InputFormat::Tsv),
    ] {
        let counts: Vec<usize> = lines
            .iter()
            .map(|line| line.split(delim).count())
            .filter(|count| *count > 1)
            .collect();
        if counts.len() >= 2 {
            let first = counts[0];
            let stable = counts.iter().filter(|count| **count == first).count();
            if stable * 100 >= counts.len() * 90 {
                return format;
            }
        }
    }

    let log_like = lines
        .iter()
        .filter(|line| {
            line.contains(" INFO ")
                || line.contains(" WARN ")
                || line.contains(" ERROR ")
                || line.contains(" DEBUG ")
                || line.starts_with("INFO ")
                || line.starts_with("WARN ")
                || line.starts_with("ERROR ")
        })
        .count();
    if lines.len() >= 5 && log_like * 100 >= lines.len() * 50 {
        return InputFormat::Logs;
    }

    InputFormat::BinaryFallback
}
