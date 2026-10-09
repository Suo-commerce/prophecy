//! The checked-in schema contract: `schema/prophecy.schema.json`
//! must match what the crate's types generate TODAY. A stale file
//! fails with a diff naming the drift; regenerate with
//! `cargo run --bin gen-schema > schema/prophecy.schema.json`.

use std::path::Path;

#[test]
fn schema_up_to_date() {
    let schema = schemars::schema_for!(prophecy::Prophecy);
    let regenerated = serde_json::to_string_pretty(&schema).unwrap();

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join("prophecy.schema.json");
    let checked_in = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e} — the schema file must be checked in",
            path.display()
        )
    });

    // Compare semantically (both are pretty-printed by the same
    // generator, but parse so key order can never cause a false
    // positive) and report a line diff on drift.
    let checked: serde_json::Value = serde_json::from_str(&checked_in)
        .expect("the checked-in schema is valid JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&regenerated).expect("the regenerated schema is valid JSON");
    if checked == fresh {
        return;
    }

    // The diff: first differing line pairs, bounded.
    let old: Vec<&str> = checked_in.lines().collect();
    let new: Vec<&str> = regenerated.lines().collect();
    let mut diff = String::from("schema/prophecy.schema.json is STALE — regenerate with `cargo run --bin gen-schema > schema/prophecy.schema.json`\n\n");
    let mut shown = 0;
    for (n, (a, b)) in old.iter().zip(new.iter()).enumerate() {
        if a != b {
            diff.push_str(&format!("  line {}:\n    checked-in: {a}\n    regenerated: {b}\n", n + 1));
            shown += 1;
            if shown >= 10 {
                diff.push_str("  …\n");
                break;
            }
        }
    }
    if old.len() != new.len() {
        diff.push_str(&format!(
            "  length differs: checked-in {} lines, regenerated {} lines\n",
            old.len(),
            new.len()
        ));
    }
    panic!("{diff}");
}
