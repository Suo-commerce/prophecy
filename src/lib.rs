//! The prophecy crate — SPEC.md v0.2.
//!
//! A **prophecy** is a statement about something that *could*
//! happen, produced by an automated process, that only comes true
//! if a human acts on it. Producers watch sources and *prophesy*;
//! humans *fulfil* or *dismiss*; nothing a producer writes ever
//! acts on its own.
//!
//! The crate holds the record and the state rules, nothing else
//! (§9): no storage mandate, no transport, no authorisation —
//! hosts bind those. Three invariants recur throughout:
//!
//! - **Producers only append.** No producer may change a
//!   prophecy's state; the only state changes come from a human
//!   decision, host bookkeeping, or time.
//! - **Fulfilment is a human act, always.** A model may *ask* that
//!   a prophecy be fulfilled; the asking is never the fulfilling.
//! - **Identity is established, not claimed.** `producer` is
//!   stamped by the host from how the record arrived — a
//!   submitter cannot set, propose or override it, which is why
//!   `ProphecySubmission` carries none of the host fields.
//!
//! The normative wire contract is `schema/prophecy.schema.json`
//! (generated from these types, kept current by test). Where this
//! documentation and the schema disagree, the schema wins.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A prophecy's identity. Host-assigned (ULID recommended —
/// sortable by creation); a submitter never provides one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ProphecyId(pub String);

impl ProphecyId {
    /// A fresh host-side id (UUID v4; hosts may pass their own ULID
    /// string to [`Prophecy::from_submission`] instead).
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

/// Who appended the prophecy, as **established by the host** from
/// how the record arrived (in-process reader, verified socket peer,
/// human channel) — never from the record itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Producer {
    /// Deterministic code, no inference. Readers produce only
    /// Findings (§4); a Reader-origin record of any other kind is
    /// rejected at append.
    #[default]
    Reader,
    /// A model. May ask for fulfilment; the asking is never the
    /// fulfilling. Dismissal by a model is host-opted-in only (§5).
    Model,
    /// A human — the operator or a reviewer.
    Human,
}

/// What kind of proposal this is. Content is carried by the
/// record's top-level fields (`title`, `body`, `fulfilment`), not
/// by the variant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Something observed: a new message, a moved meeting, a
    /// changed row. No inference; `confidence` is 1.0 and
    /// `fulfilment` is usually absent.
    #[default]
    Finding,
    /// Proposed text. The text lives in `body`; writing it anywhere
    /// is the fulfilment, not the prophecy.
    Draft,
    /// A proposed operation: `fulfilment.target` names it,
    /// `fulfilment.params` parameterise it.
    SuggestedAction,
    /// A proposed change to the producing system itself. A pull
    /// request; a human merges it — no host applies it
    /// automatically, and a producer never acts on its own.
    Improvement,
}

/// The lifecycle state. All terminal states are terminal: there is
/// no reopening and no second transition (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Open. Entered at creation; left only by a human decision,
    /// host bookkeeping, or expiry.
    Proposed,
    /// Terminal. A human approved; the host carries out the
    /// fulfilment as ordinary, gated, supervised work.
    Fulfilled,
    /// Terminal. Declined by a human, by host bookkeeping with a
    /// machine-written note, or by a model where the host has
    /// explicitly opted in (the model's identity is then mandatory
    /// in `resolution_note`).
    Dismissed,
    /// Terminal. `expires_at` passed while still open; the host's
    /// sweep moved it here.
    Expired,
}

/// What the producer was looking at: the configured source plus a
/// stable per-row identity (§3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    /// The configured source this came from (the host's source id).
    pub id: String,
    /// Stable identity of the item across observations,
    /// producer-defined (for mail: sender + subject + received;
    /// for a calendar: the event's natural key). Deduplication
    /// applies only to an identical `(row_key, row_hash)` pair; a
    /// changed `row_hash` under the same `row_key` is a content
    /// change and produces a NEW prophecy whose evidence references
    /// the prior one's id.
    pub row_key: String,
    /// SHA-256, hex-encoded, over the producer's canonical
    /// serialisation of the item at observation time. Compared only
    /// within one source on one host.
    pub row_hash: String,
}

/// The audit trail back to what was observed (§3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// When the evidence was captured.
    pub captured_at: DateTime<Utc>,
    /// An open map the producer fills so the prophecy can be
    /// audited back to what was observed. Populo writes
    /// `observation_hash` and `event_log_seq`; a repository
    /// producer might write a commit id; a chat producer a message
    /// id. The schema does not enumerate keys, and no key carries
    /// a semantic obligation for consumers. A model-origin record
    /// derived from Findings references those Findings' ids here
    /// (§6.4); hosts should verify the referenced ids exist.
    #[serde(default)]
    pub evidence: BTreeMap<String, serde_json::Value>,
}

/// What fulfilling this prophecy means, on a `Prophecy` (§3).
/// `target` and `params` come from the submitter;
/// `requires_human` is **host-stamped** and must not be accepted
/// from a submitter — submitters use
/// [`SubmissionFulfilment`], which has no such field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fulfilment {
    /// A recipe, command or action name the host understands.
    pub target: String,
    /// Parameters for the target, from the submitter.
    #[serde(default = "default_params")]
    pub params: serde_json::Value,
    /// HOST-STAMPED: true whenever the host accepted a fulfilment
    /// with the record, false when the record has none. It has
    /// exactly one meaning — *there is a fulfilment to perform* —
    /// and is never a trust claim (derivation trust is the
    /// host-verified `producer`). A submitter cannot set it.
    pub requires_human: bool,
}

/// The submitter's form of [`Fulfilment`] (§8): `target` and
/// `params` only. No `requires_human` — the host derives and
/// stamps that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubmissionFulfilment {
    /// A recipe, command or action name the host understands.
    pub target: String,
    /// Parameters for the target.
    #[serde(default = "default_params")]
    pub params: serde_json::Value,
}

fn default_params() -> serde_json::Value {
    serde_json::json!({})
}

/// One prophecy record (§3). Host-stamped fields (`id`, `producer`,
/// `state`, `created_at`, `expires_at`, `state_changed_at`,
/// `resolution_note`, `fulfilment.requires_human`) are written by
/// the host and are not accepted from a submitter; submitter fields
/// (`kind`, `source`, `provenance`, `title`, `body`,
/// `confidence`, `fulfilment.target`, `fulfilment.params`) arrive
/// via [`ProphecySubmission`] and are stamped in by
/// [`Prophecy::from_submission`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Prophecy {
    /// **Host-stamped.** Unique, host-assigned (ULID recommended —
    /// sortable by creation).
    pub id: ProphecyId,
    /// **Submitter-provided.** Finding · Draft · SuggestedAction ·
    /// Improvement.
    pub kind: Kind,
    /// **Host-stamped** from the authenticated caller identity
    /// (§7.5). A submitter cannot set, propose or override it.
    pub producer: Producer,
    /// **Submitter-provided.** The configured source plus stable
    /// row identity.
    pub source: Source,
    /// **Submitter-provided.** Capture time plus open evidence map.
    pub provenance: Provenance,
    /// **Submitter-provided.** Human-readable one-liner.
    pub title: String,
    /// **Submitter-provided.** Full content: for a Draft, the
    /// draft text itself; for an Improvement, the proposal and the
    /// evidence.
    pub body: String,
    /// **Submitter-provided.** The producer's own estimate, 0.0–1.0.
    /// Readers emit 1.0 (they observed, they did not infer). It is
    /// provenance, not a ranking: presentation order is the host's
    /// concern and must not be derived from this field alone.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    /// **Submitter-provided plan, host-completed.** Present when
    /// fulfilling this prophecy means running something; absent
    /// for informational Findings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fulfilment: Option<Fulfilment>,
    /// **Host-stamped.** Proposed · Fulfilled · Dismissed ·
    /// Expired. Only the host changes it (§5).
    #[serde(default = "default_proposed")]
    pub state: State,
    /// **Host-stamped.**
    pub created_at: DateTime<Utc>,
    /// **Host-stamped** at creation (default 24 h; a submitter may
    /// propose a shorter value and the host clamps it). Never
    /// extended.
    pub expires_at: DateTime<Utc>,
    /// **Host-stamped.** When `state` last changed — the creation
    /// instant while Proposed.
    pub state_changed_at: DateTime<Utc>,
    /// **Host-stamped.** Required on `Fulfilled` and `Dismissed`;
    /// says who decided and why. Machine-written for host
    /// bookkeeping (§7.6); a model dismissal carries the model's
    /// identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

fn default_proposed() -> State {
    State::Proposed
}

/// The shape a producer sends to a host (§8): **only submitter
/// fields**. No `id`, no `producer`, no `state`, no `created_at`,
/// no `state_changed_at`, no `requires_human`, no
/// `resolution_note` — the host stamps all of those
/// ([`Prophecy::from_submission`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProphecySubmission {
    /// **Submitter-provided.** Finding · Draft · SuggestedAction ·
    /// Improvement.
    pub kind: Kind,
    /// **Submitter-provided.** The configured source plus stable
    /// row identity.
    pub source: Source,
    /// **Submitter-provided.** Capture time plus open evidence map.
    pub provenance: Provenance,
    /// **Submitter-provided.** Human-readable one-liner.
    pub title: String,
    /// **Submitter-provided.** Full content.
    pub body: String,
    /// **Submitter-provided.** The producer's own estimate, 0.0–1.0.
    /// Readers emit 1.0 (they observed, they did not infer). It is
    /// provenance, not a ranking: presentation order is the host's
    /// concern and must not be derived from this field alone.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    /// **Submitter-provided.** The fulfilment plan minus
    /// `requires_human`, which the host stamps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fulfilment: Option<SubmissionFulfilment>,
    /// Proposed expiry. The host clamps it, never extends it; the
    /// stamped `expires_at` is the host's word, not the
    /// submitter's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<ExpiresIn>")]
    pub expires_in: Option<chrono::Duration>,
}

/// Schema vehicle for `expires_in` (never serialised itself):
/// `chrono::Duration` has no schemars impl, so this mirrors its
/// serde wire form — the two-element array `[seconds, nanos]`.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct ExpiresIn(pub chrono::Duration);

impl schemars::JsonSchema for ExpiresIn {
    fn schema_name() -> String {
        "ExpiresIn".to_string()
    }
    fn json_schema(_generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{ArrayValidation, InstanceType, Metadata, SchemaObject, SingleOrVec};
        let integer = SchemaObject {
            instance_type: Some(InstanceType::Integer.into()),
            ..Default::default()
        };
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some(
                    "chrono Duration in its serde wire form: [seconds, nanos]".into(),
                ),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(SingleOrVec::Vec(vec![
                    integer.clone().into(),
                    integer.into(),
                ])),
                min_items: Some(2),
                max_items: Some(2),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

/// Schema vehicle only — never serialised. Carries both wire
/// shapes at the top level of `schema/prophecy.schema.json` so
/// consumers see `Prophecy` and `ProphecySubmission` (and every
/// shared definition) in one file.
#[doc(hidden)]
#[derive(Debug, Clone, JsonSchema)]
pub struct SchemaRoot {
    pub prophecy: Prophecy,
    pub submission: ProphecySubmission,
}

#[derive(Debug, thiserror::Error)]
pub enum ProphecyError {
    #[error("prophecy {0} is already {1:?} — all terminal states are terminal; no second transition")]
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
    /// `requires_human` has exactly one meaning: *there is a
    /// fulfilment to perform* (§6.3). Hosts present it from this
    /// accessor — the value is host-stamped inside
    /// `fulfilment` when a plan exists, and false when there is
    /// no plan. It is never a trust claim.
    pub fn requires_human(&self) -> bool {
        self.fulfilment.as_ref().is_some_and(|f| f.requires_human)
    }

    /// Build a stored record from a submission: the ONLY way
    /// submitter fields become a prophecy. The host supplies the
    /// identity it established (§7.5) and both timestamps; the
    /// submission's `expires_in` proposal has already been
    /// clamped into `expires_at` by the caller.
    ///
    /// Stamps: `requires_human` = `submission.fulfilment.is_some()`
    /// (true only when a plan came with the record),
    /// `state: Proposed`, `state_changed_at: created_at`,
    /// `resolution_note: None`.
    pub fn from_submission(
        id: String,
        producer: Producer,
        submission: ProphecySubmission,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        let fulfilment = submission.fulfilment.map(|f| Fulfilment {
            target: f.target,
            params: f.params,
            requires_human: true,
        });
        Self {
            id: ProphecyId(id),
            kind: submission.kind,
            producer,
            source: submission.source,
            provenance: submission.provenance,
            title: submission.title,
            body: submission.body,
            confidence: submission.confidence,
            fulfilment,
            state: State::Proposed,
            created_at,
            expires_at,
            state_changed_at: created_at,
            resolution_note: None,
        }
    }

    /// Record-level validation (§6, applied identically whatever
    /// path wrote the record — channel validation is a separate,
    /// earlier concern, §7.5):
    /// - Readers produce only Findings; a Reader-origin record of
    ///   any other kind is rejected (§4).
    /// - `confidence` is within 0.0–1.0.
    /// - The evidence map is non-empty (a prophecy with empty
    ///   evidence is a bug in the producer, §6.2).
    /// - The source carries a full row identity (§6.5).
    pub fn validate(&self) -> Result<(), ProphecyError> {
        if matches!(self.producer, Producer::Reader) && !matches!(self.kind, Kind::Finding) {
            return Err(ProphecyError::Invalid(format!(
                "readers produce only Findings; a Reader-origin {:?} is rejected at append",
                self.kind
            )));
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(ProphecyError::Invalid(format!(
                "confidence {} is outside 0.0–1.0",
                self.confidence
            )));
        }
        if self.provenance.evidence.is_empty() {
            return Err(ProphecyError::Invalid(
                "provenance.evidence is empty — every prophecy must be traceable to what was observed (§6.2)".into(),
            ));
        }
        if self.source.id.is_empty()
            || self.source.row_key.is_empty()
            || self.source.row_hash.is_empty()
        {
            return Err(ProphecyError::Invalid(
                "source needs id, row_key and row_hash (§6.5 stable identity)".into(),
            ));
        }
        Ok(())
    }

    /// Resolve: `Proposed → Fulfilled|Dismissed` (a human decision
    /// or opted-in model dismissal — the caller's channel question,
    /// not the record's) or `Proposed → Expired` (host sweep).
    /// Terminal, one-way, expiry-checked; the note is required on
    /// Fulfilled and Dismissed (§3: it says who decided and why).
    pub fn resolve(&mut self, state: State, note: Option<String>) -> Result<(), ProphecyError> {
        if self.state != State::Proposed {
            return Err(ProphecyError::AlreadyResolved(self.id.clone(), self.state));
        }
        if let State::Proposed = state {
            return Err(ProphecyError::Invalid(
                "resolve moves a record OUT of Proposed; Proposed is the open state".into(),
            ));
        }
        if matches!(state, State::Fulfilled | State::Dismissed) && note.is_none() {
            return Err(ProphecyError::Invalid(
                "resolution_note is required on Fulfilled and Dismissed — who decided, and why".into(),
            ));
        }
        if Utc::now() > self.expires_at {
            return Err(ProphecyError::Expired(self.id.clone(), self.expires_at));
        }
        self.state = state;
        self.state_changed_at = Utc::now();
        self.resolution_note = note;
        Ok(())
    }

    /// Open means Proposed (§5). The expiry sweep may still be
    /// owed; hosts run it before presenting.
    pub fn is_open(&self) -> bool {
        self.state == State::Proposed
    }
}

/// The reference store: append-only from the producer side,
/// durable as one JSONL file (mode 0600), resolution as the only
/// mutation, expiry sweep, model-origin rate cap, and coalescing
/// that never drops a record (§7).
///
/// Storage details are the host's business (§9) — this is one
/// correct implementation, not a mandate.
/// Deterministic row_hash for a coalescing summary: SHA-256, hex,
/// over the coalesced id list (the summary's canonical item).
fn summary_row_hash(ids: &[String]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    for id in ids {
        hasher.update(id.as_bytes());
        hasher.update(b"\n");
    }
    hex_or_fallback(hasher.finalize())
}

fn hex_or_fallback(digest: sha2::digest::Output<sha2::Sha256>) -> String {
    let bytes = digest.as_slice();
    // hex encode without a hex dep
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[derive(Debug, Default)]
pub struct ProphecyStore {
    records: Vec<Prophecy>,
    quarantined: Vec<(usize, String)>,
}

impl ProphecyStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one prophecy (validates first — record-level, §6).
    /// The producing side's only write.
    pub fn append(&mut self, p: Prophecy) -> Result<(), ProphecyError> {
        p.validate()?;
        self.records.push(p);
        Ok(())
    }

    /// Append within the model-origin budget: `Err` when the
    /// count of `Producer::Model` records created in the last
    /// hour already equals `max_model_per_hour` (§7.6 — rate
    /// limits apply to model-origin prophecies; reader findings
    /// are coalesced, never dropped).
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

    /// Coalesce open Findings from one source into a single
    /// summary Finding (§7.6 — never drop). The coalesced records
    /// end **Dismissed** with a machine-written `resolution_note`
    /// naming the summary id; the summary's evidence references
    /// every coalesced id. Returns the summary for the caller to
    /// append. Fewer than two open findings from the source: no
    /// coalescing.
    pub fn coalesce_findings(&mut self, source_id: &str) -> Option<Prophecy> {
        let now = Utc::now();
        let grouped: Vec<usize> = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.is_open() && matches!(r.kind, Kind::Finding) && r.source.id == source_id
            })
            .map(|(i, _)| i)
            .collect();
        if grouped.len() < 2 {
            return None;
        }
        let summary_id = ProphecyId::new();
        let coalesced_ids: Vec<String> = grouped
            .iter()
            .map(|&i| self.records[i].id.0.clone())
            .collect();
        let newest = grouped
            .iter()
            .map(|&i| &self.records[i])
            .max_by_key(|r| r.provenance.captured_at)
            .expect("grouped non-empty");
        let mut evidence = newest.provenance.evidence.clone();
        evidence.insert(
            "coalesced_ids".into(),
            serde_json::json!(coalesced_ids),
        );
        let summary = Prophecy {
            id: summary_id.clone(),
            kind: Kind::Finding,
            producer: Producer::Reader,
            source: Source {
                id: source_id.to_string(),
                row_key: format!("summary:{source_id}"),
                // The summary's "item" is the set it replaces: the
                // digest is over the coalesced id list, so the same
                // set coalesced twice yields the same row_hash.
                row_hash: summary_row_hash(&coalesced_ids),
            },
            provenance: Provenance {
                captured_at: newest.provenance.captured_at,
                evidence,
            },
            title: format!(
                "{} findings coalesced (source {source_id})",
                grouped.len()
            ),
            body: format!(
                "Coalesced summary of {} open findings from source {source_id}; ids in evidence.coalesced_ids.",
                grouped.len()
            ),
            confidence: 1.0,
            fulfilment: None,
            state: State::Proposed,
            created_at: now,
            expires_at: newest.expires_at,
            state_changed_at: now,
            resolution_note: None,
        };
        for &i in &grouped {
            let p = &mut self.records[i];
            p.state = State::Dismissed;
            p.state_changed_at = now;
            p.resolution_note = Some(format!(
                "coalesced into summary {}",
                summary_id
            ));
        }
        Some(summary)
    }

    pub fn get(&self, id: &ProphecyId) -> Option<&Prophecy> {
        self.records.iter().find(|r| &r.id == id)
    }

    /// Resolve by id (terminal, expiry-checked, note required on
    /// Fulfilled/Dismissed).
    pub fn resolve(
        &mut self,
        id: &ProphecyId,
        state: State,
        note: Option<String>,
    ) -> Result<(), ProphecyError> {
        let p = self
            .records
            .iter_mut()
            .find(|r| &r.id == id)
            .ok_or_else(|| ProphecyError::NotFound(id.clone()))?;
        p.resolve(state, note)
    }

    /// Open (Proposed) prophecies.
    pub fn open(&self) -> impl Iterator<Item = &Prophecy> {
        self.records.iter().filter(|r| r.is_open())
    }

    /// The expiry sweep: Proposed records past `expires_at` move to
    /// `Expired` with a machine-written note. Returns the swept
    /// count. Resolved records are never touched (§5).
    pub fn sweep_expired(&mut self) -> usize {
        let now = Utc::now();
        let mut swept = 0;
        for p in &mut self.records {
            if p.is_open() && now > p.expires_at {
                p.state = State::Expired;
                p.state_changed_at = now;
                p.resolution_note = Some("expiry sweep".into());
                swept += 1;
            }
        }
        swept
    }

    /// Durable save: one JSONL file, one record per line, mode
    /// 0600 (owner-only — bodies carry watched content).
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

    /// Load from the JSONL file. Malformed lines fail the load —
    /// hosts preferring quarantine use
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

    /// Load with quarantine: malformed lines move to
    /// `<path>.corrupt` (mode 0600) and are recorded in
    /// [`ProphecyStore::quarantined`] — the healthy records load,
    /// the corrupt ones are preserved for inspection, nothing is
    /// silently dropped and the host does not fail to boot on one
    /// bad line.
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
    use chrono::Duration as ChrDuration;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    /// A reader Finding submission — the B1 shape a watcher emits.
    fn finding_submission() -> ProphecySubmission {
        ProphecySubmission {
            kind: Kind::Finding,
            source: Source {
                id: "watch-1".into(),
                row_key: "sender=a@b.example subject=hello received=2026-10-09T09:15:00Z".into(),
                row_hash: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".into(),
            },
            provenance: Provenance {
                captured_at: now(),
                evidence: BTreeMap::from([
                    ("observation_hash".into(), serde_json::json!("67d1659bfe71b89d50b45a4ad1a9e5b9")),
                    ("event_log_seq".into(), serde_json::json!(42)),
                ]),
            },
            title: "New message from a@b.example: hello".into(),
            body: "Unread message - sender a@b.example, subject hello, received 09:15".into(),
            confidence: 1.0,
            fulfilment: None,
            expires_in: None,
        }
    }

    fn from_sub(sub: ProphecySubmission, producer: Producer) -> Prophecy {
        let created = now();
        Prophecy::from_submission(
            ProphecyId::new().0,
            producer,
            sub,
            created,
            created + ChrDuration::hours(24),
        )
    }

    // ---- §3: submission shape ----

    #[test]
    fn from_submission_with_fulfilment_requires_human() {
        let sub = ProphecySubmission {
            fulfilment: Some(SubmissionFulfilment {
                target: "compose_email".into(),
                params: serde_json::json!({ "to": "a@b.example" }),
            }),
            ..finding_submission()
        };
        let p = from_sub(sub, Producer::Model);
        assert!(p.requires_human(), "a plan present means requires_human");
        assert!(p.fulfilment.as_ref().expect("plan present").requires_human);
        // Host stamps the rest.
        assert_eq!(p.state, State::Proposed);
        assert_eq!(p.resolution_note, None);
        assert_eq!(p.state_changed_at, p.created_at);
        assert_eq!(p.fulfilment.as_ref().unwrap().target, "compose_email");
    }

    #[test]
    fn from_submission_without_fulfilment_is_informational() {
        let p = from_sub(finding_submission(), Producer::Reader);
        assert!(
            !p.requires_human(),
            "no plan means requires_human is false (host-stamped derivation)"
        );
        assert!(p.fulfilment.is_none());
        assert_eq!(p.state, State::Proposed);
    }

    #[test]
    fn submission_round_trips_serde() {
        let sub = ProphecySubmission {
            kind: Kind::SuggestedAction,
            expires_in: Some(ChrDuration::seconds(5400)),
            fulfilment: Some(SubmissionFulfilment {
                target: "compose_email".into(),
                params: serde_json::json!({ "to": "a@b.example" }),
            }),
            ..finding_submission()
        };
        let json = serde_json::to_string(&sub).unwrap();
        let back: ProphecySubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sub);
        assert_eq!(back.expires_in, Some(ChrDuration::seconds(5400)));
        // The chrono Duration wire form: [seconds, nanos].
        assert!(json.contains("[5400,0]"), "chrono Duration serde shape: {json}");
    }

    #[test]
    fn submission_carries_only_submitter_fields() {
        // §8: the submission carries ONLY submitter fields. None of
        // the host-stamped fields may appear in its JSON at all.
        let sub = finding_submission();
        let v = serde_json::to_value(&sub).unwrap();
        for forbidden in [
            "id", "producer", "state", "created_at", "expires_at",
            "state_changed_at", "resolution_note", "requires_human",
        ] {
            assert!(
                v.get(forbidden).is_none(),
                "submission JSON must not carry host field `{forbidden}`: {v}"
            );
        }
        // And the nested plan has no requires_human either.
        let with_plan = ProphecySubmission {
            fulfilment: Some(SubmissionFulfilment {
                target: "x".into(),
                params: serde_json::json!({}),
            }),
            ..finding_submission()
        };
        let v = serde_json::to_value(&with_plan).unwrap();
        assert!(v["fulfilment"].get("requires_human").is_none(), "{v}");
        for forbidden in ["id", "producer", "state", "resolution_note"] {
            assert!(v.get(forbidden).is_none(), "{forbidden} leaked: {v}");
        }
    }

    // ---- §4: readers produce only Findings ----

    #[test]
    fn readers_produce_only_findings() {
        let p = from_sub(finding_submission(), Producer::Reader);
        p.validate().unwrap();
        let draft = ProphecySubmission {
            kind: Kind::Draft,
            ..finding_submission()
        };
        let err = from_sub(draft, Producer::Reader).validate().unwrap_err();
        assert!(err.to_string().contains("only Findings"), "{err}");
        // A model may produce any kind, including a Finding.
        from_sub(finding_submission(), Producer::Model).validate().unwrap();
    }

    // ---- §6: record-level validation ----

    #[test]
    fn validation_rejects_empty_evidence_and_bad_confidence() {
        let mut p = from_sub(finding_submission(), Producer::Reader);
        p.provenance.evidence.clear();
        assert!(p.validate().unwrap_err().to_string().contains("evidence"));
        let mut p = from_sub(finding_submission(), Producer::Reader);
        p.confidence = 1.5;
        assert!(p.validate().unwrap_err().to_string().contains("confidence"));
        let mut p = from_sub(finding_submission(), Producer::Reader);
        p.source.row_key.clear();
        assert!(p.validate().unwrap_err().to_string().contains("row_key"));
    }

    // ---- §5: states and transitions ----

    #[test]
    fn resolution_is_terminal() {
        let mut p = from_sub(finding_submission(), Producer::Reader);
        p.resolve(State::Fulfilled, Some("human approved via hold dialog".into()))
            .unwrap();
        let err = p
            .resolve(State::Dismissed, Some("changed my mind".into()))
            .unwrap_err();
        assert!(matches!(err, ProphecyError::AlreadyResolved(_, _)), "{err}");
    }

    #[test]
    fn resolution_note_required_on_fulfilled_and_dismissed() {
        let mut p = from_sub(finding_submission(), Producer::Reader);
        let err = p.resolve(State::Fulfilled, None).unwrap_err();
        assert!(err.to_string().contains("resolution_note"), "{err}");
        let mut p = from_sub(finding_submission(), Producer::Reader);
        let err = p.resolve(State::Dismissed, None).unwrap_err();
        assert!(err.to_string().contains("resolution_note"), "{err}");
    }

    #[test]
    fn expired_prophecy_cannot_resolve() {
        let created = now();
        let mut p = Prophecy::from_submission(
            ProphecyId::new().0,
            Producer::Reader,
            finding_submission(),
            created,
            created - ChrDuration::hours(1),
        );
        let err = p.resolve(State::Fulfilled, Some("too late".into())).unwrap_err();
        assert!(matches!(err, ProphecyError::Expired(_, _)), "{err}");
        // The sweep moves it to Expired instead.
        let mut store = ProphecyStore::new();
        store.append(p).unwrap();
        assert_eq!(store.sweep_expired(), 1);
        assert_eq!(store.open().count(), 0);
    }

    // ---- §7.6: host obligations on the store ----

    #[test]
    fn per_hour_budget_counts_model_origin_only() {
        let mut store = ProphecyStore::new();
        for _ in 0..5 {
            store
                .append_capped(from_sub(finding_submission(), Producer::Reader), 2)
                .unwrap();
        }
        for _ in 0..2 {
            store
                .append_capped(from_sub(finding_submission(), Producer::Model), 2)
                .unwrap();
        }
        let err = store
            .append_capped(from_sub(finding_submission(), Producer::Model), 2)
            .unwrap_err();
        assert!(err.to_string().contains("budget"), "{err}");
    }

    #[test]
    fn coalescing_never_drops_and_ends_dismissed() {
        // RE-5 / §7.6: coalesced records end Dismissed with a
        // machine-written note NAMING the summary id; the summary
        // references every coalesced id in its evidence.
        let mut store = ProphecyStore::new();
        let mut ids = Vec::new();
        for i in 0..3 {
            let sub = ProphecySubmission {
                source: Source {
                    row_key: format!("row-{i}"),
                    ..finding_submission().source
                },
                ..finding_submission()
            };
            let p = from_sub(sub, Producer::Reader);
            ids.push(p.id.clone());
            store.append(p).unwrap();
        }
        let summary = store.coalesce_findings("watch-1").expect("3 findings coalesce");
        let summary_id = summary.id.clone();
        store.append(summary).unwrap();

        for id in &ids {
            let p = store.get(id).expect("still present - never dropped");
            assert_eq!(p.state, State::Dismissed);
            let note = p.resolution_note.as_deref().expect("machine note");
            assert!(note.contains(&summary_id.0), "note names the summary id: {note}");
        }
        let open: Vec<&Prophecy> = store.open().collect();
        assert_eq!(open.len(), 1, "one open summary finding");
        let summary = open[0];
        assert_eq!(summary.kind, Kind::Finding);
        assert_eq!(summary.confidence, 1.0);
        assert!(summary.fulfilment.is_none());
        let coalesced = summary.provenance.evidence["coalesced_ids"]
            .as_array()
            .expect("evidence references coalesced ids");
        assert_eq!(coalesced.len(), 3);
        for id in &ids {
            assert!(coalesced.contains(&serde_json::json!(id.0)));
        }
        // Fewer than two open: no coalescing.
        assert!(store.coalesce_findings("watch-1").is_none());
    }

    #[test]
    fn store_round_trips_durably_with_owner_only_mode() {
        let dir = std::env::temp_dir().join(format!("prophecy-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip-prophecies.jsonl");
        let mut store = ProphecyStore::new();
        let mut p = from_sub(finding_submission(), Producer::Reader);
        p.resolve(State::Dismissed, Some("not now - jarmo".into()))
            .unwrap();
        store.append(p).unwrap();
        store.append(from_sub(finding_submission(), Producer::Reader)).unwrap();
        store.save(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let back = ProphecyStore::load(&path).unwrap();
        assert_eq!(back.records.len(), 2);
        assert_eq!(back.open().count(), 1);
    }

    #[test]
    fn malformed_lines_quarantined_not_dropped() {
        let dir = std::env::temp_dir().join(format!("prophecy-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quarantine-prophecies.jsonl");
        let good = from_sub(finding_submission(), Producer::Reader);
        let good_line = serde_json::to_string(&good).unwrap();
        let text = format!("{good_line}\n{{ this is not json\n{good_line}\n");
        std::fs::write(&path, text).unwrap();
        let store = ProphecyStore::load_with_quarantine(&path).unwrap();
        assert_eq!(store.records.len(), 2);
        assert_eq!(store.quarantined().len(), 1);
        assert_eq!(store.quarantined()[0].0, 2);
        let qpath = path.with_extension("corrupt");
        let quarantined_text = std::fs::read_to_string(&qpath).unwrap();
        assert!(quarantined_text.contains("this is not json"));
    }

    // ---- schema ----

    #[test]
    fn schema_covers_both_wire_shapes() {
        let root = schemars::schema_for!(SchemaRoot);
        let v = serde_json::to_value(&root).unwrap();
        // Both types are $ref-ed at the top level, and their
        // definitions are exported.
        let refs = [
            v["properties"]["prophecy"]["$ref"].as_str().unwrap_or_default(),
            v["properties"]["submission"]["$ref"].as_str().unwrap_or_default(),
        ];
        assert!(refs[0].contains("Prophecy"), "{refs:?}");
        assert!(refs[1].contains("ProphecySubmission"), "{refs:?}");
        let defs = v["definitions"].as_object().expect("definitions exported");
        for name in [
            "Prophecy", "ProphecySubmission", "Kind", "Producer",
            "Source", "Provenance", "Fulfilment", "SubmissionFulfilment",
            "State", "ProphecyId",
        ] {
            assert!(defs.contains_key(name), "definition {name} missing");
        }
        // §10: consumers reject unknown kind/producer/state values —
        // the schema enumerates them.
        assert!(serde_json::to_string(&defs["Producer"]).unwrap().contains("reader"));
        assert!(serde_json::to_string(&defs["State"]).unwrap().contains("proposed"));
        // confidence is bounded in both shapes.
        for path in [
            &v["definitions"]["Prophecy"]["properties"]["confidence"],
            &v["definitions"]["ProphecySubmission"]["properties"]["confidence"],
        ] {
            assert!(path.get("minimum").is_some(), "confidence carries minimum: {path}");
            assert!(path.get("maximum").is_some(), "confidence carries maximum: {path}");
        }
        // The open evidence map: additionalProperties, no
        // enumerated keys (§3).
        let prov = &v["definitions"]["Provenance"]["properties"]["evidence"];
        assert!(
            prov.get("additionalProperties").is_some()
                || prov.get("type").and_then(|t| t.as_str()) == Some("object"),
            "evidence is an open map: {prov}"
        );
    }
}
