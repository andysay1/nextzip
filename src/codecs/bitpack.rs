pub fn bits_required(max: u64) -> u8 {
    if max == 0 {
        1
    } else {
        (64 - max.leading_zeros()) as u8
    }
}

pub fn encode(values: &[u64], bits: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buffer = 0u128;
    let mut used = 0u8;
    for value in values {
        buffer |= (*value as u128) << used;
        used += bits;
        while used >= 8 {
            out.push(buffer as u8);
            buffer >>= 8;
            used -= 8;
        }
    }
    if used > 0 {
        out.push(buffer as u8);
    }
    out
}

pub fn decode(bytes: &[u8], bits: u8, count: usize) -> Vec<u64> {
    let mask = if bits == 64 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    let mut out = Vec::with_capacity(count);
    let mut buffer = 0u128;
    let mut used = 0u8;
    let mut idx = 0;
    while out.len() < count {
        while used < bits && idx < bytes.len() {
            buffer |= (bytes[idx] as u128) << used;
            used += 8;
            idx += 1;
        }
        out.push((buffer & mask) as u64);
        buffer >>= bits;
        used = used.saturating_sub(bits);
    }
    out
}
