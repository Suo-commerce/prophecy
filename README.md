# prophecy

Shared types for unattended-work systems: a **prophecy** is a
statement about something *could* happen, produced by an automated
process, that only comes true if a human acts on it. Producers
watch sources and prophesy; humans fulfil or dismiss; nothing a
producer writes ever acts on its own. See `SPEC.md` (v0.2) — the
normative wire contract is `schema/prophecy.schema.json`, and
where the document and the schema disagree, the schema wins.

**The model in five sentences.** A record carries submitter
fields (`kind`, `source`, `provenance`, `title`, `body`,
`confidence`, an optional `fulfilment` plan) and host-stamped
fields (`id`, `producer`, `state`, timestamps, `resolution_note`,
`fulfilment.requires_human`) — submitters send a
`ProphecySubmission`, and only `Prophecy::from_submission` turns
it into a prophecy, so no producer can ever claim an identity, a
state, or a trust level. Producers append only: the only state
changes come from a human decision, host bookkeeping, or the
expiry sweep. `requires_human` has exactly one meaning — *there
is a fulfilment to perform* — stamped true when a plan came with
the record and false when it did not; it is never a trust claim
(derivation trust is the host-verified `producer`). All terminal
states are terminal: Fulfilled, Dismissed and Expired are ends
with no reopening, and `resolution_note` says who decided and
why. Readers produce only Findings, model-origin records are
rate-capped, and coalescing folds findings into a summary that
references every coalesced id — never dropping anything.

## State diagram

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

All three terminal states are terminal: no reopening, no second
transition. A model may *ask* for fulfilment; the asking is never
the fulfilling — hosts turn a model's request into a prompt on a
human channel (SPEC §7.4).

## Example

A reader-produced Finding — informational (no fulfilment plan →
`requires_human` false), validating against
`schema/prophecy.schema.json`:

```json
{
  "id": "0b8c1f5e-9d3a-4c71-9f2b-6a4e8d7c1a20",
  "kind": "finding",
  "producer": "reader",
  "source": {
    "id": "watch-1",
    "row_key": "sender=a@b.example subject=hello received=2026-10-09T09:15:00Z",
    "row_hash": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
  },
  "provenance": {
    "captured_at": "2026-10-09T09:15:02Z",
    "evidence": {
      "observation_hash": "67d1659bfe71b89d50b45a4ad1a9e5b9",
      "event_log_seq": 42
    }
  },
  "title": "New message from a@b.example: hello",
  "body": "Unread message - sender a@b.example, subject 'hello', received 2026-10-09T09:15:00Z",
  "confidence": 1.0,
  "state": "proposed",
  "created_at": "2026-10-09T09:15:02Z",
  "expires_at": "2026-10-10T09:15:02Z",
  "state_changed_at": "2026-10-09T09:15:02Z"
}
```

The submission a producer sends to obtain such a record (only
submitter fields — the host stamps everything else):

```json
{
  "kind": "suggested_action",
  "source": {
    "id": "watch-1",
    "row_key": "sender=a@b.example subject=hello received=2026-10-09T09:15:00Z",
    "row_hash": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
  },
  "provenance": {
    "captured_at": "2026-10-09T09:16:00Z",
    "evidence": {
      "derived_from": "0b8c1f5e-9d3a-4c71-9f2b-6a4e8d7c1a20"
    }
  },
  "title": "Draft a reply to a@b.example: hello",
  "body": "Thanks for the note - I'll get back to you today.",
  "confidence": 0.8,
  "fulfilment": {
    "target": "compose_email",
    "params": { "to": "a@b.example" }
  },
  "expires_in": [7200, 0]
}
```

The host stamps that submission into a record with
`producer: "model"`, `requires_human: true` inside the fulfilment
plan, `state: "proposed"`, and its own timestamps.

## Embedding

The crate is dependency-isolated (no host-system crates) and
carries only the record and the state rules (SPEC §9): storage
format, transport and authorisation are the host's business, with
`ProphecyStore` as one correct reference implementation. The
public surface is `Prophecy`, `ProphecySubmission`, `Kind`,
`Producer`, `Source`, `Provenance`, `Fulfilment`,
`SubmissionFulfilment`, `State`; the wire contract is
`schema/prophecy.schema.json` (Draft-07, both shapes at the top
level). Regenerate after any type change:

```sh
cargo run --bin gen-schema > schema/prophecy.schema.json
```

`tests/schema_up_to_date.rs` fails with a diff when the
checked-in schema is stale.

License: Apache-2.0, holder Suo-commerce.
