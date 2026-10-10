# Prophecies — Specification

**Version:** 0.2 (2026-10-09) · **Status:** draft
**Tracks:** crate rev `73e1f81` for the record shape; the submission shape (§8) lands in the next crate rev
**Normative artefact:** `schema/prophecy.schema.json` in this repository. Where this document and the schema disagree, the schema wins and this document has a bug.
**Changes from 0.1:** host-stamped identity (RE-1), model dismissal opt-in and surfaced (RE-2), re-observation rule (RE-3), `requires_human` given one meaning (RE-4), coalesced terminal state named (RE-5), §5/§7.5 reconciled (RE-6), editorial E-1–E-6. All from the Hermes review of 0.1.

---

## 1. The model

A **prophecy** is a statement about something that *could* happen, produced by an automated process, that only comes true if a human acts on it. Producers watch sources and **prophesy**; humans **fulfil** or **dismiss**; nothing a producer writes ever acts on its own.

That is the whole mechanism. It exists so that an autonomous process can be useful while unattended — gathering, ranking, drafting, proposing — without ever holding the authority to change anything. The record format is deliberately neutral: a prophecy can come from a desktop-automation service reading an inbox through the accessibility tree, from an agent harness reading a repository, or from a human typing one in. Consumers treat them identically.

Three consequences follow from the one rule above and recur throughout:

- **Producers only append.** No producer may change a prophecy's state. The only state changes come from a human decision, from host bookkeeping, or from the passage of time.
- **Fulfilment is a human act, always.** A model may *ask* that a prophecy be fulfilled; the asking is never the fulfilling.
- **Identity is established, not claimed.** Who produced a prophecy is something the host knows from how the record arrived, never something the record says about itself.

## 2. Vocabulary

| Term | Meaning |
|---|---|
| prophecy | One record: a finding, a draft, a suggested action, or a proposed improvement, with provenance |
| prophecies | The collection a host presents to its human |
| prophesy (verb) | To produce and append a prophecy |
| fulfil | A human approves a prophecy; the host then carries out its fulfilment as ordinary, gated, supervised work |
| dismiss | A human — or a model, only where the host has opted in (§5) — declines a prophecy; nothing runs |
| expire | The prophecy's `expires_at` passed while it was still open |
| producer | Who appended the prophecy, as established by the host: a deterministic reader, a model, or a human |
| source | What the producer was looking at, with a stable per-row identity |
| host | The system that stores prophecies and presents them (Populo, Hermes, Layover, …) |

Spelling: *prophecy / prophecies* is the noun, *prophesy* is the verb. Code uses `prophecy` for the type and `prophecies` for the collection.

## 3. The record

Field semantics; types and constraints are in the schema. Fields marked **host** are written by the host and are not accepted from a submitter (§8).

| Field | Who | Meaning |
|---|---|---|
| `id` | host | Unique, host-assigned (ULID recommended — sortable by creation) |
| `kind` | submitter | `Finding` · `Draft` · `SuggestedAction` · `Improvement` — see §4 |
| `producer` | **host** | `Reader` (deterministic code, no inference) · `Model` · `Human`. Stamped from the authenticated caller identity (§7.5). A submitter cannot set, propose or override it |
| `source` | submitter | `id` (the configured source this came from), `row_key` (stable identity of the item across observations, producer-defined), `row_hash` (SHA-256, hex, over the producer's canonical serialization of the item at observation time) |
| `provenance` | submitter | `captured_at` plus `evidence`: an open map the producer fills so the prophecy can be audited back to what was observed. Populo writes `observation_hash` and `event_log_seq`; a repository producer might write a commit id; a chat producer a message id. The schema does not enumerate keys, and no key carries a semantic obligation for consumers (§10) |
| `title` / `body` | submitter | Human-readable. `body` carries the full content: for a Draft, the draft text itself |
| `confidence` | submitter | 0.0–1.0, the producer's own estimate. Readers emit 1.0 (they observed, they did not infer). It is provenance, not a ranking: how prophecies are ordered or grouped for presentation is the host's concern and must not be derived from this field alone |
| `fulfilment` | submitter, host-completed | Optional. What fulfilling this prophecy means: `target` (a recipe, command or action name the host understands) and `params` from the submitter; `requires_human` stamped by the host (§6.3) |
| `state` | host | `Proposed` · `Fulfilled` · `Dismissed` · `Expired` — see §5 |
| `created_at` / `expires_at` / `state_changed_at` | host | `expires_at` is set by the host at creation (default 24 h); a submitter may propose a shorter value and the host clamps it. Never extended |
| `resolution_note` | host | Required on `Fulfilled` and `Dismissed`; says who decided and why. For a model dismissal, the model's identity is mandatory. For host bookkeeping, machine-written (§7.6) |

## 4. Kinds

- **Finding** — something observed. A new message, a moved meeting, a changed row. Produced by readers without inference; `confidence` is 1.0 and `fulfilment` is usually absent (a finding is informational). Findings are the raw material for everything else. **Readers produce only Findings**; a `Reader`-origin record of any other kind is rejected at append.
- **Draft** — proposed text. A reply, a summary, a message. The text lives in `body`; writing it anywhere is the fulfilment, not the prophecy.
- **SuggestedAction** — a proposed operation: `fulfilment.target` names it, `fulfilment.params` parameterise it. "Archive these four", "reply to X with draft Y".
- **Improvement** — a proposed change to the producing system itself: a profile, a recipe, a source definition. `fulfilment.target` names the thing to change; `body` carries the proposal and the evidence. **No host may apply an Improvement automatically.** It is a pull request; a human merges it.

## 5. States and who may change them

```
            ┌──────────────┐
            │   Proposed   │
            └──┬────┬───┬──┘
      human    │    │   │   time
    approves   │    │   │   (expires_at passed)
               ▼    │   ▼
        Fulfilled   │  Expired
                    │
        human,      ▼
     opted-in    Dismissed
     model, or
     host
     bookkeeping
```

- All three terminal states are terminal. There is no reopening and no second transition.
- **Proposed → Fulfilled**: only by a human decision, delivered over a channel the host trusts to be human (a desktop dialog, an authenticated messaging callback, a sideband with its own auth). Never by a model-visible call alone.
- **Proposed → Dismissed**: by a human; by host bookkeeping (coalescing, §7.6) with a machine-written note; or by a model **only where the host has explicitly opted in** — dismissal runs nothing, so it is strictly weaker than fulfilment, but it is a visibility channel: a model that dismisses everything hides everything. Hosts must not grant it by default; when granted, the model's identity goes in `resolution_note` and the host surfaces model-dismissed records to the human in the next presentation as dismissed on their behalf.
- **Proposed → Expired**: by the host's sweep, when `expires_at` has passed. Resolved records are never touched by the sweep.
- A transition on an already-terminal or expired record fails. Racing an expiry sweep against a resolution is the host's problem to serialise; whichever wins, the record ends terminal and the other path fails closed.

## 6. Producer obligations

1. **Append only.** A producer has no API to a prophecy's state.
2. **Fill provenance.** Every prophecy must be traceable to what was observed. A prophecy with empty evidence is a bug in the producer.
3. **`requires_human`** has exactly one meaning: *there is a fulfilment to perform*. The host stamps it `true` whenever `fulfilment` is present and `false` whenever it is absent (an informational Finding). It is never a trust claim — derivation trust is carried by the host-verified `producer`. Hosts may present `requires_human: false` records collapsed and may expire them on a shorter clock. A submitter cannot set it.
4. **Reference what you consumed.** A model-origin Draft, SuggestedAction or Improvement derived from Findings must reference those Findings' ids in its evidence; hosts should verify the referenced ids exist.
5. **Stable identity and re-observation.** `row_key` must identify the same item across observations (for mail: sender + subject + received; for a calendar: the event's natural key). Deduplication applies only to an identical `(row_key, row_hash)` pair: the same item observed unchanged produces nothing. The same `row_key` with a changed `row_hash` is a content change: the producer appends a **new** prophecy whose evidence references the prior prophecy's `id`; the prior record's state is untouched, whatever it is (an already-fulfilled move that moves again surfaces again). `row_hash` is compared only within one source on one host, so cross-host hash agreement is not required.
6. **No self-modification.** A producer never acts on its own Improvement.

## 7. Host obligations

1. **Store append-only from the producer side; durable; expiry sweep.** Resolution is the only mutation.
2. **Present** open prophecies to the human with provenance, and give the human fulfil / dismiss on a human-trusted channel. Include anything dismissed on their behalf since they last looked (§5).
3. **Fulfilment runs as ordinary supervised work.** Approving a prophecy starts whatever the host's normal, gated, attended execution path is. Fulfilment grants no permission the host would not grant the same request made directly.
4. **A model's request to fulfil becomes a question to a human**, not an action. If a host exposes a resolve call to models, `Fulfilled` from that call must raise a prompt on the human channel and do nothing else. The prompt lapsing unanswered returns the prophecy to open — it does not dismiss it; only an explicit human decision dismisses. (A prompt guarding a *proposal* costs nothing to keep open; this differs from a prompt guarding a *dispatch*, which a host may fail closed.) The host bounds repeated requests on the same still-open id.
5. **Establish identity; then validate the record.** The host stamps `producer` from authenticated caller identity — a process-local reader, a socket peer verified by the transport (peer credentials, a session token), a human channel — and rejects any submission that attempts to carry one. Record-level validation (§6.3–6.4) is then applied identically whatever path wrote the record. These are two different things and both are required: record validation is channel-independent; identity, including human-decision authentication (§5), is necessarily a property of the channel. Trusting a channel *instead of* validating the record adds a thing to audit without removing a check; validating a record whose identity was self-asserted checks nothing.
6. **Never drop.** Rate limits apply to model-origin prophecies. Reader findings that exceed a presentation threshold are coalesced into a summary Finding that references every coalesced id; the coalesced records end **`Dismissed`** with a machine-written `resolution_note` naming the summary id. This is host bookkeeping, not a decision, and is the only non-human, non-model source of `Dismissed`.
7. **No learning loop.** Prophecies are task-scoped working state with expiry. A host does not build profiles from them or feed them back into its own behaviour. Long-term memory, if any, lives elsewhere and reads prophecies as input.

## 8. Protocol surface

Three operations and one internal actor, named abstractly; each host binds them to its own transport.

| Operation | Who | Effect |
|---|---|---|
| `list(filter)` | anyone the host admits | Open prophecies, bounded summaries, provenance; dismissed-on-your-behalf since last presentation |
| `submit(submission)` | producers | The submission carries only submitter fields (§3): kind, source, provenance, title, body, confidence, fulfilment target/params, optional proposed expiry. The host stamps id, producer, `requires_human`, timestamps, state. Validate (§6), append. Never changes existing state |
| `resolve(id, decision, note)` | humans; models only for `Dismissed` and only where opted in | `Dismissed`: terminal. `Fulfilled` from a human channel: terminal, fulfilment starts. `Fulfilled` from a model: raises a human prompt, record unchanged |
| expiry sweep | host-internal | `Proposed → Expired` when `expires_at` has passed |

Populo's binding: MCP tools `prophecies_list`, `prophecies_submit`, `prophecy_resolve` on its Unix socket, with socket peers stamped `Model`, in-process readers stamped `Reader`, and the hold dialog / Telegram callback stamped `Human`. Other hosts choose their own.

## 9. Out of scope

Deliberately not specified here, because they are the host's business: storage format and location; transport, authentication and authorisation of callers (only that identity must be established, §7.5); how the human channel is implemented; what `fulfilment.target` names resolve to; rate-limit values; presentation order and grouping; how sources are configured. The schema and this document fix only the record and the state rules, so that a prophecy written by one host is legible to another.

## 10. Versioning

The schema carries the version. Additive changes (new optional fields, new evidence keys) are minor; a new evidence key must not carry a semantic obligation for consumers, or it is a major change in disguise. Removing a field, adding a kind, a producer or a state, or changing a transition rule is a major change. Consumers must ignore unknown fields and must reject unknown `kind`, `producer` or `state` values.
