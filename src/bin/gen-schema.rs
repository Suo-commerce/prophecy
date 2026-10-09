//! Regenerates `schema/prophecy.schema.json` from the crate's
//! types. Run from the crate root after any type change:
//!
//! ```sh
//! cargo run --bin gen-schema > schema/prophecy.schema.json
//! ```
//!
//! `tests/schema_up_to_date.rs` fails with a diff when the
//! checked-in file is stale — regenerate rather than hand-edit.

use prophecy::Prophecy;

fn main() {
    let schema = schemars::schema_for!(Prophecy);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
