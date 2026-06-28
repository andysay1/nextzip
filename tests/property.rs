use nextzip::{pack, unpack, PackOptions};
use proptest::prelude::*;
use proptest::test_runner::TestCaseResult;

fn roundtrip(input: &[u8]) -> TestCaseResult {
    let archive = pack(
        input,
        PackOptions {
            exact: false,
            level: 1,
        },
    )
    .unwrap();
    let restored = unpack(&archive).unwrap();
    prop_assert_eq!(restored, input);
    Ok(())
}

proptest! {
    #[test]
    fn jsonl_roundtrip_random_rows(rows in prop::collection::vec((0i64..1_000_000, 0u8..8, "[a-z]{1,8}"), 20..200)) {
        let mut input = String::new();
        for (idx, (ts, user, action)) in rows.into_iter().enumerate() {
            input.push_str(&format!(
                r#"{{"idx":{},"ts":{},"user":{},"action":"{}"}}"#,
                idx, ts, user, action
            ));
            input.push('\n');
        }
        roundtrip(input.as_bytes())?;
    }

    #[test]
    fn csv_roundtrip_random_rows(rows in prop::collection::vec((0i64..1_000_000, 0u8..8, "[a-z]{1,8}"), 1..200), crlf in any::<bool>()) {
        let newline = if crlf { "\r\n" } else { "\n" };
        let mut input = format!("ts,user,action{newline}");
        for (ts, user, action) in rows {
            input.push_str(&format!("{ts},{user},{action}{newline}"));
        }
        roundtrip(input.as_bytes())?;
    }

    #[test]
    fn log_roundtrip_random_rows(rows in prop::collection::vec((0u32..86_400, 0u16..512, 0u16..512, 0u16..1000), 5..200)) {
        let levels = ["INFO", "WARN", "ERROR"];
        let actions = ["view", "buy", "search"];
        let mut input = String::new();
        for (idx, (sec, user, item, latency)) in rows.into_iter().enumerate() {
            input.push_str(&format!(
                "2026-01-01T12:{:02}:{:02}Z {} user={} action={} item={} latency={}\n",
                (sec / 60) % 60,
                sec % 60,
                levels[idx % levels.len()],
                user,
                actions[idx % actions.len()],
                item,
                latency
            ));
        }
        roundtrip(input.as_bytes())?;
    }
}
