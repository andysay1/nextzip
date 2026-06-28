pub fn blake3_bytes(input: &[u8]) -> [u8; 32] {
    *blake3::hash(input).as_bytes()
}
