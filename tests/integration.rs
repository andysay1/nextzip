use std::fs;
use std::process::Command;

use nextzip::{detect_format, inspect_archive, pack, unpack, InputFormat, PackOptions};

#[test]
fn detect_jsonl() {
    let input = (0..25)
        .map(|i| {
            format!(
                r#"{{"action":"view","ts":{},"user":{}}}"#,
                1710000000 + i,
                i % 5
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(detect_format(input.as_bytes()), InputFormat::Jsonl);
}

#[test]
fn detect_csv() {
    assert_eq!(detect_format(b"a,b,c\n1,2,3\n4,5,6\n"), InputFormat::Csv);
}

#[test]
fn pack_unpack_jsonl() {
    let input = (0..100)
        .map(|i| {
            format!(
                r#"{{"action":"view","ts":{},"user":{}}}"#,
                1710000000 + i,
                i % 7
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let restored = unpack(&archive).unwrap();
    assert_eq!(restored, input.as_bytes());
}

#[test]
fn inspect_shows_column_plan() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                r#"{{"action":"view","item":{},"ts":{},"user":{}}}"#,
                i % 100,
                1710000000 + i,
                i % 7
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("format: Jsonl"));
    assert!(report.contains("- action:"));
}

#[test]
fn structural_jsonl_selects_multiple_codecs() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                r#"{{"action":"{}","item":{},"price":{},"ts":{},"user":{}}}"#,
                if i % 4 == 0 { "buy" } else { "view" },
                9000 + (i % 100),
                99 + (i % 20),
                1710000000 + i,
                101 + (i % 8)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("fallback: false"));
    assert!(report.contains("Dictionary"));
    assert!(report.contains("Delta") || report.contains("FrameOfReference"));
}

#[test]
fn multi_block_jsonl_roundtrips() {
    let input = (0..40_000)
        .map(|i| {
            format!(
                r#"{{"action":"{}","region":"r{}","ts":{},"user":{}}}"#,
                if i % 5 == 0 { "buy" } else { "view" },
                i % 12,
                1710000000 + i,
                1000 + (i % 32)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    assert_eq!(unpack(&archive).unwrap(), input.as_bytes());
}

#[test]
fn repeated_string_columns_use_dictionary() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                r#"{{"action":"{}","country":"{}","ts":{}}}"#,
                ["view", "buy", "cart"][i % 3],
                ["US", "DE", "FR", "JP"][i % 4],
                1710000000 + i
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("Dictionary"));
    assert_eq!(unpack(&archive).unwrap(), input.as_bytes());
}

#[test]
fn linear_integer_columns_use_delta_of_delta() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                r#"{{"action":"view","seq":{},"ts":{}}}"#,
                1_000_000 + i * 10,
                1_710_000_000 + i
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("DeltaOfDelta"));
    assert_eq!(unpack(&archive).unwrap(), input.as_bytes());
}

#[test]
fn exact_mode_uses_fallback_and_roundtrips() {
    let input = br#"{"b":2,"a":1}
{"b":3,"a":2}
"#;
    let archive = pack(
        input,
        PackOptions {
            exact: true,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("fallback: true"));
    assert_eq!(unpack(&archive).unwrap(), input);
}

#[test]
fn exact_jsonl_can_use_structural_raw_line_residual() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                r#"{{ "z" : {} , "a" : "{}" }}"#,
                i % 8,
                ["view", "buy", "search"][i % 3]
            )
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: true,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("fallback: false"));
    assert!(report.contains("__nxz_raw_line"));
    assert_eq!(unpack(&archive).unwrap(), input.as_bytes());
}

#[test]
fn pack_unpack_csv() {
    let input = b"ts,user,action\n1,42,view\n2,43,buy\n";
    let archive = pack(
        input,
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let restored = unpack(&archive).unwrap();
    assert_eq!(restored, input);
}

#[test]
fn pack_unpack_csv_crlf_structural() {
    let input = b"ts,user,action\r\n1,42,view\r\n2,43,buy\r\n3,44,view\r\n";
    let archive = pack(
        input,
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let restored = unpack(&archive).unwrap();
    assert_eq!(restored, input);
}

#[test]
fn pack_unpack_logs() {
    let input = b"2026-01-01 INFO user=42 action=view item=991\n2026-01-01 INFO user=43 action=buy item=992\n2026-01-01 ERROR user=42 action=view item=993\n2026-01-01 WARN user=44 action=view item=994\n2026-01-01 INFO user=45 action=view item=995\n";
    let archive = pack(
        input,
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let restored = unpack(&archive).unwrap();
    assert_eq!(restored, input);
}

#[test]
fn logs_use_template_columns() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                "2026-01-01T12:{:02}:{:02}Z {} user={} action={} item={} latency={}",
                (i / 60) % 60,
                i % 60,
                ["INFO", "WARN", "ERROR"][i % 3],
                1000 + (i % 128),
                ["view", "buy", "search"][i % 3],
                9000 + (i % 256),
                25 + (i % 700)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let archive = pack(
        input.as_bytes(),
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let report = inspect_archive(&archive).unwrap();
    assert!(report.contains("- timestamp:"));
    assert!(report.contains("- level:"));
    assert!(report.contains("- latency:"));
    assert_eq!(unpack(&archive).unwrap(), input.as_bytes());
}

#[test]
fn fallback_roundtrip_random() {
    let input: Vec<u8> = (0..2048).map(|i| ((i * 73 + 19) % 251) as u8).collect();
    let archive = pack(
        &input,
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    let restored = unpack(&archive).unwrap();
    assert_eq!(restored, input);
}

#[test]
fn bench_outputs_report() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("data.jsonl");
    fs::write(
        &input,
        (0..50)
            .map(|i| format!(r#"{{"action":"view","ts":{},"user":{}}}"#, i, i % 3))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nextzip"))
        .arg("bench")
        .arg(&input)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("zstd:"));
    assert!(stdout.contains("gzip:"));
    assert!(stdout.contains("nextzip:"));
}

#[test]
fn dict_roundtrip() {
    let values = vec!["a".to_string(), "b".to_string(), "a".to_string()];
    let (dict, indexes) = nextzip::codecs::dict::encode(&values);
    assert_eq!(nextzip::codecs::dict::decode(&dict, &indexes), values);
}

#[test]
fn delta_roundtrip() {
    let values = vec![1000, 1001, 1004, 1007];
    assert_eq!(
        nextzip::codecs::delta::decode(&nextzip::codecs::delta::encode(&values)),
        values
    );
}

#[test]
fn delta2_roundtrip() {
    let values = vec![1000, 1010, 1020, 1031];
    assert_eq!(
        nextzip::codecs::delta2::decode(&nextzip::codecs::delta2::encode(&values)),
        values
    );
}

#[test]
fn rle_roundtrip() {
    let values = vec!["a", "a", "a", "b", "b", "c"];
    assert_eq!(
        nextzip::codecs::rle::decode(&nextzip::codecs::rle::encode(&values)),
        values
    );
}

#[test]
fn bitpack_roundtrip() {
    let values = vec![0, 1, 3, 7, 2, 5, 6];
    let bits = nextzip::codecs::bitpack::bits_required(*values.iter().max().unwrap());
    assert_eq!(
        nextzip::codecs::bitpack::decode(
            &nextzip::codecs::bitpack::encode(&values, bits),
            bits,
            values.len()
        ),
        values
    );
}
