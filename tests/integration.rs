use std::fs;
use std::process::Command;

use nextzip::{
    detect_format, inspect_archive, pack, pack_file, unpack, unpack_file, InputFormat, PackOptions,
};

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
fn manual_header_codec_roundtrips_archive_metadata() {
    let input = (0..100)
        .map(|i| format!(r#"{{"action":"view","ts":{},"user":{}}}"#, i, i % 4))
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
    assert!(report.contains("header_schema: 1"));
    assert!(report.contains("format: Jsonl"));
    assert!(report.contains("archive_size:"));
    assert!(report.contains("compression_ratio:"));
    assert!(report.contains("size_breakdown:"));
    assert!(report.contains("payload_compressed:"));
    assert_eq!(unpack(&archive).unwrap(), input.as_bytes());
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
fn logs_support_mixed_template_field_order() {
    let input = (0..20_000)
        .map(|i| {
            if i % 2 == 0 {
                format!(
                    "2026-01-01T12:{:02}:{:02}Z INFO user={} action=view item={}",
                    (i / 60) % 60,
                    i % 60,
                    1000 + (i % 128),
                    9000 + (i % 256)
                )
            } else {
                format!(
                    "2026-01-01T12:{:02}:{:02}Z WARN item={} latency={} user={}",
                    (i / 60) % 60,
                    i % 60,
                    9000 + (i % 256),
                    25 + (i % 700),
                    1000 + (i % 128)
                )
            }
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
    assert!(report.contains("__nxz_field_order"));
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
fn inspect_outputs_block_codec_stats() {
    let input = (0..20_000)
        .map(|i| {
            format!(
                r#"{{"action":"{}","ts":{},"user":{}}}"#,
                ["view", "buy"][i % 2],
                1_710_000_000 + i,
                i % 16
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
    assert!(report.contains("block_codec_stats:"));
    assert!(report.contains("chunks="));
}

#[test]
fn bench_directory_outputs_markdown_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    fs::create_dir(&data).unwrap();
    fs::write(
        data.join("a.jsonl"),
        (0..50)
            .map(|i| format!(r#"{{"action":"view","ts":{},"user":{}}}"#, i, i % 3))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    fs::write(data.join("b.csv"), b"ts,user\n1,2\n3,4\n").unwrap();
    let json = dir.path().join("results.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nextzip"))
        .arg("bench")
        .arg(&data)
        .arg("--json")
        .arg(&json)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("| file | format |"));
    let rows: serde_json::Value = serde_json::from_slice(&fs::read(&json).unwrap()).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
}

#[test]
fn inspect_cli_outputs_json() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("events.jsonl");
    let archive = dir.path().join("events.nxz");
    fs::write(
        &input,
        (0..20_000)
            .map(|i| format!(r#"{{"action":"view","ts":{},"user":{}}}"#, i, i % 4))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();

    let pack_output = Command::new(env!("CARGO_BIN_EXE_nextzip"))
        .arg("pack")
        .arg(&input)
        .arg(&archive)
        .output()
        .unwrap();
    assert!(pack_output.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_nextzip"))
        .arg("inspect")
        .arg(&archive)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["format"], "Jsonl");
    assert!(json["archive_size"].as_u64().unwrap() > 0);
    assert!(
        json["size_breakdown"]["payload_compressed"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(!json["block_codec_stats"].as_array().unwrap().is_empty());
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

#[test]
fn file_api_streams_binary_fallback_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("blob.bin");
    let archive = dir.path().join("blob.nxz");
    let restored = dir.path().join("blob.out");
    let data = (0..(2 * 1024 * 1024))
        .map(|i| ((i * 37 + i / 251) % 256) as u8)
        .collect::<Vec<_>>();
    fs::write(&input, &data).unwrap();

    pack_file(
        &input,
        &archive,
        PackOptions {
            exact: false,
            level: 3,
        },
    )
    .unwrap();
    unpack_file(&archive, &restored).unwrap();

    assert_eq!(fs::read(restored).unwrap(), data);
}
