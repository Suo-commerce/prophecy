//! Regenerates `schema/prophecy.schema.json` from the crate's
//! types. Run from the crate root after any type change:
//!
//! ```sh
//! cargo run --bin gen-schema > schema/prophecy.schema.json
//! ```
//!
//! The schema is generated from [`prophecy::SchemaRoot`], the
//! never-serialised vehicle that carries both wire shapes
//! (`Prophecy`, `ProphecySubmission`) at the top level with every
//! shared definition exported. `tests/schema_up_to_date.rs`
//! fails with a diff when the checked-in file is stale —
//! regenerate rather than hand-edit.

fn main() {
    let schema = schemars::schema_for!(prophecy::SchemaRoot);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
