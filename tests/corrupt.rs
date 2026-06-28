use nextzip::{inspect_archive, pack, unpack, PackOptions};

fn archive_for(input: &[u8]) -> Vec<u8> {
    pack(
        input,
        PackOptions {
            exact: false,
            level: 1,
        },
    )
    .unwrap()
}

#[test]
fn rejects_bad_magic() {
    let mut archive = archive_for(b"not,enough\n1,2\n");
    archive[0..4].copy_from_slice(b"BAD!");
    assert!(unpack(&archive).is_err());
}

#[test]
fn rejects_truncated_archive() {
    let archive = archive_for(b"a,b\n1,2\n3,4\n");
    for len in 0..archive.len().min(32) {
        assert!(unpack(&archive[..len]).is_err());
    }
}

#[test]
fn rejects_checksum_mismatch() {
    let mut archive = archive_for(
        (0..100)
            .map(|i| format!(r#"{{"ts":{},"user":{}}}"#, 1_710_000_000 + i, i % 4))
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    );
    let last = archive.len() - 1;
    archive[last] ^= 0xff;
    assert!(unpack(&archive).is_err());
    assert!(inspect_archive(&archive).is_err());
}
