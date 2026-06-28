pub fn zigzag_i64(x: i64) -> u64 {
    ((x << 1) ^ (x >> 63)) as u64
}

pub fn unzigzag_u64(x: u64) -> i64 {
    ((x >> 1) as i64) ^ (-((x & 1) as i64))
}
