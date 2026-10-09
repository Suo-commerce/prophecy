//! The prophecy crate — shared types for unattended-work systems:
//! what a watcher *notices* (findings), what it *proposes* (drafts,
//! suggested actions, improvements), and how those proposals *end*
//! (fulfilled, dismissed, or expired), each with provenance.
//!
//! Design contract:
//!
//! - **A prophecy is a proposal, never an act.** Nothing in this
//!   crate executes anything; fulfilment is a *record* that a
//!   gated task ran — the enforcement lives in the host system,
//!   not here.
//! - **Provenance is mandatory.** Every prophecy carries the
//!   evidence it was derived from: an observation hash, a log
//!   sequence number, and who derived it (deterministic reader vs
//!   model vs human).
//! - **Resolution is terminal and one-way.** `Fulfilled` and
//!   `Dismissed` are ends, not state changes; a resolved prophecy
//!   is never re-resolved (the resolver fails closed).
//! - **`requires_human` defaults to true** and is only ever set
//!   false by the deterministic Finding path — the flag records
//!   that the finding's *derivation* needed no model, never that
//!   its fulfilment needs no human.
//!
//! This crate is dependency-isolated: no host-system crates, no
//! screen or domain vocabulary in the types. Consumers embed the
//! types as-is; the JSON Schema in `schema/prophecy.schema.json`
//! is the wire contract.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A prophecy's identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ProphecyId(pub String);

impl ProphecyId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ProphecyId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProphecyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What produced this prophecy — the trust tier of the derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Producer {
    /// A deterministic reader (configured code, no inference).
    /// The only producer that may set `requires_human: false`.
    Reader,
    /// A model over collected findings (resident or external).
    /// Always `requires_human: true`.
    Model,
    /// A human (the operator, a reviewer).
    Human,
}

/// The origin of one observation a prophecy was derived from —
/// the audit trail. Host systems fill these fields from their own
/// records; the crate only requires they be present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// The observation's content hash — the exact state the
    /// evidence was read from (whatever hash the host records).
    pub observation_hash: String,
    /// The host's log sequence number for that observation — the
    /// prophecy points INTO the durable log.
    pub event_log_seq: u64,
    /// When the evidence was captured.
    pub captured_at: DateTime<Utc>,
}

/// A deterministic watch target's identity — where a finding came
/// from (a configured source id plus its stable identity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    /// The configured source's id (the watcher's own name for it).
    pub id: String,
    /// The finding's stable ROW identity as the reader defines it
    /// (e.g. sender+subject+received for a message list, or a
    /// natural key for any row-shaped source). Readers key their
    /// emissions on this so a host can deduplicate and coalesce.
    pub row_key: String,
    /// The full-row hash — the evidence for THIS row of the
    /// finding (row contents hashed; any content change is a new
    /// evidence value).
    pub row_hash: String,
}

/// The kinds of prophecy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// A deterministic observation from a reader: a structured
    /// fact about some watched state. No inference.
    Finding {
        /// The source that produced it.
        source: Source,
        /// The finding's payload — the reader's structured output
        /// (rows, counts, matched shapes). Opaque to this crate.
        payload: serde_json::Value,
    },
    /// A drafted response to a finding. A model product; the text
    /// itself, never a dispatch.
    Draft {
        /// The finding this drafts a response to.
        finding: ProphecyId,
        /// The drafted text.
        text: String,
    },
    /// A suggested action with its recipe shape. A proposal —
    /// nothing executes it.
    SuggestedAction {
        /// The recipe (or named procedure) the action proposes.
        recipe: String,
        /// The proposed params (rendered, not templated).
        params: serde_json::Value,
        /// Why the model suggests it (bounded, recorded).
        rationale: String,
    },
    /// An improvement proposal pointing at a configuration target
        /// with evidence. NOTHING applies it — a human edits the file.
    Improvement {
        /// The target it points at (a file, section, or config key).
        target: String,
        /// The proposed change, as text or a patch-shaped object.
        proposal: serde_json::Value,
    },
}

impl Default for Kind {
    fn default() -> Self {
        Kind::Finding {
            source: Source {
                id: String::new(),
                row_key: String::new(),
                row_hash: String::new(),
            },
            payload: serde_json::Value::Null,
        }
    }
}

/// How a prophecy ended. Terminal: a fulfilled or dismissed
/// prophecy is never re-resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fulfilment {
    /// The terminal state this prophecy ended in.
    pub state: State,
    /// Free-text note (the operator's or producer's reason).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// When it was resolved.
    pub resolved_at: DateTime<Utc>,
    /// Who resolved it.
    pub resolved_by: Producer,
}

/// The terminal state of a resolved prophecy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The proposal's gated task ran to completion. This is a
    /// RECORD of completion, never an instruction.
    Fulfilled,
    /// Rejected without running.
    Dismissed,
    /// Removed by the expiry sweep (not a choice anyone made).
    Expired,
}

/// One prophecy record (the stored, append-only form).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Prophecy {
    pub id: ProphecyId,
    pub kind: Kind,
    pub producer: Producer,
    pub provenance: Provenance,
    /// False ONLY for deterministic Findings from a reader.
    /// Model- and human-produced prophecies carry true; the store
    /// REJECTS a `false` from a non-Reader producer (fail closed).
    #[serde(default = "default_true")]
    pub requires_human: bool,
    /// When the prophecy was created.
    pub created_at: DateTime<Utc>,
    /// Expiry: after this instant the prophecy may no longer be
    /// resolved — the sweep moves it to `Expired`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// The fulfilment, once it happens. Terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fulfilment: Option<Fulfilment>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, thiserror::Error)]
pub enum ProphecyError {
    #[error("prophecy {0} is already resolved ({1:?}) — resolution is terminal")]
    AlreadyResolved(ProphecyId, State),
    #[error("prophecy {0} expired at {1} — cannot resolve an expired prophecy")]
    Expired(ProphecyId, DateTime<Utc>),
    #[error("prophecy {0} not found")]
    NotFound(ProphecyId),
    #[error("invalid prophecy: {0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Prophecy {
    /// Validate the invariants at creation time:
    /// - `requires_human: false` is ONLY valid for reader-produced
    ///   Findings (fail closed at the store, not at resolve time).
    /// - Provenance must carry a non-empty observation hash.
    pub fn validate(&self) -> Result<(), ProphecyError> {
        if !self.requires_human {
            let reader_finding =
                matches!(self.producer, Producer::Reader) && matches!(self.kind, Kind::Finding { .. });
            if !reader_finding {
                return Err(ProphecyError::Invalid(format!(
                    "requires_human=false is only valid for reader-produced Findings (got {:?}/{})",
                    self.producer,
                    kind_name(&self.kind)
                )));
            }
        }
        if self.provenance.observation_hash.is_empty() {
            return Err(ProphecyError::Invalid(
                "provenance.observation_hash is empty".into(),
            ));
        }
        Ok(())
    }

    /// Resolve the prophecy — terminal, one-way, expiry-checked.
    /// Fulfilled records a completed gated task (never an
    /// instruction); Dismissed rejects it.
    pub fn resolve(
        &mut self,
        state: State,
        note: Option<String>,
        resolved_by: Producer,
    ) -> Result<(), ProphecyError> {
        if let Some(existing) = &self.fulfilment {
            return Err(ProphecyError::AlreadyResolved(self.id.clone(), existing.state));
        }
        if let Some(expiry) = self.expires_at {
            if Utc::now() > expiry {
                return Err(ProphecyError::Expired(self.id.clone(), expiry));
            }
        }
        self.fulfilment = Some(Fulfilment {
            state,
            note,
            resolved_at: Utc::now(),
            resolved_by,
        });
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.fulfilment.is_none()
    }
}

fn kind_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Finding { .. } => "finding",
        Kind::Draft { .. } => "draft",
        Kind::SuggestedAction { .. } => "suggested_action",
        Kind::Improvement { .. } => "improvement",
    }
}

/// The store: append-only from the producer side, durable across
/// host restarts (one JSONL file, one record per line), with an
/// expiry sweep. The design contract:
///
/// - `append` is the ONLY write path from the watcher/reader side;
///   it never mutates existing records.
/// - `resolve` is the only mutation, and only through
///   [`Prophecy::resolve`] (terminal, expiry-checked).
/// - The sweep moves expired OPEN prophecies to `Expired`;
///   resolved records are never touched.
/// - Malformed lines on load are QUARANTINED to a side file (see
///   [`ProphecyStore::load_with_quarantine`]), never a silent drop
///   and never a load failure.
#[derive(Debug, Default)]
pub struct ProphecyStore {
    records: Vec<Prophecy>,
    /// Malformed lines quarantined during load (line number + text).
    quarantined: Vec<(usize, String)>,
}

impl ProphecyStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one prophecy (validates first — fail closed). This is
    /// the producing side's only write.
    pub fn append(&mut self, p: Prophecy) -> Result<(), ProphecyError> {
        p.validate()?;
        self.records.push(p);
        Ok(())
    }

    /// Append within the model-origin budget: `Err` when the count
    /// of MODEL-produced prophecies created in the last hour
    /// already equals `max_per_hour`. Reader-produced Findings are
    /// NOT capped here (hosts coalesce them — see
    /// [`ProphecyStore::coalesce_findings`]); the cap applies to
    /// model-origin records, whose production costs a model call.
    pub fn append_capped(
        &mut self,
        p: Prophecy,
        max_model_per_hour: usize,
    ) -> Result<(), ProphecyError> {
        let now = Utc::now();
        let recent_model = self
            .records
            .iter()
            .filter(|r| {
                matches!(r.producer, Producer::Model) && (now - r.created_at).num_hours() < 1
            })
            .count();
        if recent_model >= max_model_per_hour {
            return Err(ProphecyError::Invalid(format!(
                "model prophecy budget exceeded: {recent_model} model-origin prophecies in the last hour (max {max_model_per_hour})"
            )));
        }
        self.append(p)
    }

    /// Coalesce reader findings: open Findings from the same source
    /// id collapse into ONE summary Finding whose payload counts
    /// them, keeping the newest provenance. Findings are never
    /// dropped — the coalesced record references every coalesced
    /// id in its payload. Returns the new summary prophecy (the
    /// caller appends it) and marks the coalesced ones resolved
    /// (State::Expired with a coalesced note — terminal, never
    /// re-resolvable).
    pub fn coalesce_findings(&mut self, source_id: &str) -> Option<Prophecy> {
        let now = Utc::now();
        let mut grouped: Vec<usize> = Vec::new();
        for (i, r) in self.records.iter().enumerate() {
            if r.is_open() {
                if let Kind::Finding { source, .. } = &r.kind {
                    if source.id == source_id {
                        grouped.push(i);
                    }
                }
            }
        }
        if grouped.len() < 2 {
            return None;
        }
        let coalesced_ids: Vec<String> = grouped
            .iter()
            .map(|&i| self.records[i].id.0.clone())
            .collect();
        let newest = grouped
            .iter()
            .map(|&i| &self.records[i])
            .max_by_key(|r| r.provenance.captured_at)
            .expect("grouped non-empty");
        let summary = Prophecy {
            id: ProphecyId::new(),
            kind: Kind::Finding {
                source: Source {
                    id: source_id.to_string(),
                    row_key: format!("summary:{source_id}"),
                    row_hash: String::new(),
                },
                payload: serde_json::json!({
                    "coalesced": true,
                    "count": coalesced_ids.len(),
                    "coalesced_ids": coalesced_ids,
                }),
            },
            producer: Producer::Reader,
            provenance: newest.provenance.clone(),
            requires_human: true,
            created_at: now,
            expires_at: newest.expires_at,
            fulfilment: None,
        };
        for &i in &grouped {
            let p = &mut self.records[i];
            p.fulfilment = Some(Fulfilment {
                state: State::Expired,
                note: Some("coalesced into summary finding".into()),
                resolved_at: now,
                resolved_by: Producer::Reader,
            });
        }
        Some(summary)
    }

    pub fn get(&self, id: &ProphecyId) -> Option<&Prophecy> {
        self.records.iter().find(|r| &r.id == id)
    }

    /// Resolve by id (terminal, expiry-checked).
    pub fn resolve(
        &mut self,
        id: &ProphecyId,
        state: State,
        note: Option<String>,
        resolved_by: Producer,
    ) -> Result<(), ProphecyError> {
        let p = self
            .records
            .iter_mut()
            .find(|r| &r.id == id)
            .ok_or_else(|| ProphecyError::NotFound(id.clone()))?;
        p.resolve(state, note, resolved_by)
    }

    /// Open (unresolved, unexpired) prophecies.
    pub fn open(&self) -> impl Iterator<Item = &Prophecy> {
        self.records.iter().filter(|r| r.is_open())
    }

    /// The expiry sweep: OPEN prophecies past their expiry move to
    /// `Expired` (note naming the sweep). Returns the swept count.
    /// Never touches resolved records.
    pub fn sweep_expired(&mut self) -> usize {
        let now = Utc::now();
        let mut swept = 0;
        for p in &mut self.records {
            if p.is_open() {
                if let Some(expiry) = p.expires_at {
                    if now > expiry {
                        p.fulfilment = Some(Fulfilment {
                            state: State::Expired,
                            note: Some("expiry sweep".into()),
                            resolved_at: now,
                            resolved_by: Producer::Reader,
                        });
                        swept += 1;
                    }
                }
            }
        }
        swept
    }

    /// Durable save: one JSONL file, one record per line, mode
    /// 0600 (owner-only — payloads can carry watched content).
    pub fn save(&self, path: &std::path::Path) -> Result<(), ProphecyError> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        for p in &self.records {
            let line = serde_json::to_string(p)?;
            writeln!(f, "{line}")?;
        }
        f.flush()?;
        Ok(())
    }

    /// Load from the JSONL file (durability across restarts).
    /// Malformed lines fail the load — fail closed. Hosts that
    /// prefer quarantine should use
    /// [`ProphecyStore::load_with_quarantine`].
    pub fn load(path: &std::path::Path) -> Result<Self, ProphecyError> {
        let text = std::fs::read_to_string(path)?;
        let mut store = Self::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let p: Prophecy = serde_json::from_str(line).map_err(|e| {
                ProphecyError::Invalid(format!("line {}: {e}", n + 1))
            })?;
            store.records.push(p);
        }
        Ok(store)
    }

    /// Load with quarantine: malformed lines are moved to
    /// `<path>.corrupt` and recorded in `self.quarantined()` — the
    /// healthy records load, the corrupt ones are preserved for
    /// inspection, nothing is silently dropped and the host does
    /// not fail to boot on one bad line.
    pub fn load_with_quarantine(path: &std::path::Path) -> Result<Self, ProphecyError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return Ok(Self::new()),
        };
        let mut store = Self::new();
        let mut bad: Vec<(usize, String)> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Prophecy>(line) {
                Ok(p) => store.records.push(p),
                Err(_) => bad.push((n + 1, line.to_string())),
            }
        }
        if !bad.is_empty() {
            let qpath = path.with_extension("corrupt");
            use std::io::Write;
            let mut f = std::fs::File::create(&qpath)?;
            for (n, line) in &bad {
                writeln!(f, "# line {n}")?;
                writeln!(f, "{line}")?;
            }
            let _ = f.flush();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&qpath, std::fs::Permissions::from_mode(0o600));
            }
        }
        store.quarantined = bad;
        Ok(store)
    }

    /// Malformed lines quarantined by the last
    /// [`ProphecyStore::load_with_quarantine`] (line number +
    /// original text). Hosts surface this in diagnostics.
    pub fn quarantined(&self) -> &[(usize, String)] {
        &self.quarantined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(requires_human: bool) -> Prophecy {
        Prophecy {
            id: ProphecyId::new(),
            kind: Kind::Finding {
                source: Source {
                    id: "watch-1".into(),
                    row_key: "row-a".into(),
                    row_hash: "h1".into(),
                },
                payload: serde_json::json!({ "unread": 3 }),
            },
            producer: Producer::Reader,
            provenance: Provenance {
                observation_hash: "abc123".into(),
                event_log_seq: 42,
                captured_at: Utc::now(),
            },
            requires_human,
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + chrono::Duration::hours(24)),
            fulfilment: None,
        }
    }

    #[test]
    fn requires_human_false_only_for_reader_findings() {
        // A reader finding may be requires_human=false.
        finding(false).validate().unwrap();
        // A MODEL-produced prophecy with requires_human=false is
        // rejected at creation (fail closed).
        let mut p = finding(false);
        p.producer = Producer::Model;
        p.kind = Kind::Draft {
            finding: ProphecyId::new(),
            text: "hi".into(),
        };
        let err = p.validate().unwrap_err();
        assert!(err.to_string().contains("requires_human"), "{err}");
        // A reader SUGGESTED_ACTION with false is also rejected.
        let mut p = finding(false);
        p.kind = Kind::SuggestedAction {
            recipe: "run-thing".into(),
            params: serde_json::json!({}),
            rationale: "because".into(),
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn resolution_is_terminal() {
        let mut p = finding(true);
        p.resolve(State::Fulfilled, Some("task ran".into()), Producer::Human)
            .unwrap();
        let err = p
            .resolve(State::Dismissed, None, Producer::Human)
            .unwrap_err();
        assert!(matches!(err, ProphecyError::AlreadyResolved(_, _)));
    }

    #[test]
    fn expired_prophecy_cannot_resolve() {
        let mut p = finding(true);
        p.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        let err = p
            .resolve(State::Fulfilled, None, Producer::Human)
            .unwrap_err();
        assert!(matches!(err, ProphecyError::Expired(_, _)));
        // The sweep marks it Expired instead.
        let mut store = ProphecyStore::new();
        store.append(p).unwrap();
        assert_eq!(store.sweep_expired(), 1);
        let swept = store.open().count();
        assert_eq!(swept, 0, "the expired prophecy is no longer open");
    }

    #[test]
    fn per_hour_budget_enforced_at_append_for_model_origin() {
        let mut store = ProphecyStore::new();
        // Reader findings never hit the model cap.
        for _ in 0..5 {
            store.append_capped(finding(false), 2).unwrap();
        }
        // Model-origin records do.
        for _ in 0..2 {
            let mut p = finding(true);
            p.producer = Producer::Model;
            p.kind = Kind::Draft {
                finding: ProphecyId::new(),
                text: "x".into(),
            };
            store.append_capped(p, 2).unwrap();
        }
        let mut p = finding(true);
        p.producer = Producer::Model;
        p.kind = Kind::Draft {
            finding: ProphecyId::new(),
            text: "over".into(),
        };
        let err = store.append_capped(p, 2).unwrap_err();
        assert!(err.to_string().contains("budget"), "{err}");
    }

    #[test]
    fn findings_coalesce_never_drop() {
        let mut store = ProphecyStore::new();
        let mut ids = Vec::new();
        for _ in 0..3 {
            let p = finding(true);
            ids.push(p.id.clone());
            store.append(p).unwrap();
        }
        let summary = store.coalesce_findings("watch-1").expect("3 findings coalesce");
        store.append(summary).unwrap();
        // The coalesced findings are terminal (coalesced note), the
        // summary is open and references every id — nothing dropped.
        for id in &ids {
            let p = store.get(id).unwrap();
            assert!(!p.is_open(), "coalesced finding is terminal");
            let note = p.fulfilment.as_ref().unwrap().note.as_deref().unwrap();
            assert!(note.contains("coalesced"), "{note}");
        }
        let open: Vec<&Prophecy> = store.open().collect();
        assert_eq!(open.len(), 1, "one summary finding open");
        let summary = open[0];
        let payload = match &summary.kind {
            Kind::Finding { payload, .. } => payload,
            _ => panic!("summary is a finding"),
        };
        assert_eq!(payload["count"], serde_json::json!(3));
        let coalesced = payload["coalesced_ids"].as_array().unwrap();
        assert_eq!(coalesced.len(), 3);
        for id in &ids {
            assert!(coalesced.contains(&serde_json::json!(id.0)));
        }
        // Fewer than two open findings: no coalescing.
        assert!(store.coalesce_findings("watch-1").is_none());
    }

    #[test]
    fn store_round_trips_durably() {
        let dir = tempfile_dir();
        let path = dir.join("prophecies.jsonl");
        let mut store = ProphecyStore::new();
        let mut p = finding(true);
        p.resolve(State::Dismissed, Some("not now".into()), Producer::Human)
            .unwrap();
        store.append(p).unwrap();
        store.append(finding(false)).unwrap();
        store.save(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the store file is owner-only");
        }
        let back = ProphecyStore::load(&path).unwrap();
        assert_eq!(back.records.len(), 2);
        assert_eq!(back.open().count(), 1, "one open, one dismissed");
    }

    #[test]
    fn malformed_lines_quarantined_not_dropped() {
        let dir = tempfile_dir();
        let path = dir.join("prophecies.jsonl");
        let good = finding(true);
        let good_line = serde_json::to_string(&good).unwrap();
        let text = format!("{good_line}\n{{ this is not json\n{good_line}\n");
        std::fs::write(&path, text).unwrap();
        let store = ProphecyStore::load_with_quarantine(&path).unwrap();
        assert_eq!(store.records.len(), 2, "the healthy records load");
        assert_eq!(store.quarantined().len(), 1, "the bad line is quarantined");
        assert_eq!(store.quarantined()[0].0, 2, "the line number is kept");
        let qpath = path.with_extension("corrupt");
        let quarantined_text = std::fs::read_to_string(&qpath).unwrap();
        assert!(quarantined_text.contains("this is not json"), "{quarantined_text}");
    }

    #[test]
    fn schema_validates_example_prophecy() {
        // The README's example prophecy validates against the
        // generated schema (a JSON-schema validation implemented
        // inline: check the required fields and enum values exist
        // in the schema).
        let schema = schemars::schema_for!(Prophecy);
        let schema_json = serde_json::to_value(&schema).unwrap();
        // The root schema describes a Prophecy.
        let example = finding(true);
        let value = serde_json::to_value(&example).unwrap();
        // Required fields present in the schema. `requires_human`
        // and the optionals carry serde defaults, so they appear in
        // `properties` rather than `required` — the example still
        // carries them.
        let required = schema_json["required"].as_array().unwrap();
        for field in ["id", "kind", "producer", "provenance", "created_at"] {
            assert!(
                required.iter().any(|r| r == field),
                "schema requires {field}"
            );
            assert!(
                value.get(field).is_some(),
                "the example carries {field}"
            );
        }
        for field in ["requires_human", "expires_at", "fulfilment"] {
            assert!(
                schema_json["properties"][field].is_object(),
                "schema describes {field}"
            );
        }
        // The producer enum accepts "reader" (schemars 0.8 renders
        // a unit-only enum as oneOf-refs; check the subschema map
        // for the reader variant — robust to either rendering).
        let producer_schema = &schema_json["properties"]["producer"];
        let producer_ok = producer_schema["enum"]
            .as_array()
            .map(|a| a.contains(&serde_json::json!("reader")))
            .unwrap_or_else(|| {
                producer_schema["oneOf"]
                    .as_array()
                    .map(|a| {
                        a.iter().any(|v| {
                            v["$ref"]
                                .as_str()
                                .map(|r| r.contains("reader"))
                                .unwrap_or(false)
                                || v["enum"]
                                    .as_array()
                                    .map(|e| e.contains(&serde_json::json!("reader")))
                                    .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
                    || schema_json["definitions"]
                        .as_object()
                        .map(|d| {
                            d.keys().any(|k| {
                                k.to_lowercase().contains("producer")
                                    && serde_json::to_string(
                                        d.get(k).expect("key present"),
                                    )
                                    .map(|s| s.contains("reader"))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
            });
        assert!(producer_ok, "the schema accepts producer=reader");
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir()
            .join(format!("prophecy-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
