use nextzip::{pack, unpack, PackOptions};

fn assert_roundtrip(input: &[u8]) {
    let archive = pack(
        input,
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    assert_eq!(unpack(&archive).unwrap(), input);
}

#[test]
fn csv_quotes_and_commas_roundtrip() {
    assert_roundtrip(b"id,name,note\n1,Alice,\"hello, world\"\n2,Bob,\"x,y,z\"\n");
}

#[test]
fn csv_escaped_quotes_roundtrip() {
    assert_roundtrip(b"id,note\n1,\"he said \"\"hi\"\"\"\n2,\"plain\"\n");
}

#[test]
fn csv_empty_fields_roundtrip() {
    assert_roundtrip(b"id,name,note\n1,,empty-name\n2,Alice,\n3,,\n");
}

#[test]
fn csv_crlf_quotes_roundtrip() {
    assert_roundtrip(
        b"id,name,note\r\n1,Alice,\"hello, world\"\r\n2,Bob,\"he said \"\"hi\"\"\"\r\n",
    );
}
