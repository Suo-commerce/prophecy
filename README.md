# prophecy

Shared types for unattended-work systems: what a watcher *notices*
(findings), what it *proposes* (drafts, suggested actions,
improvements), and how those proposals *end* (fulfilled, dismissed,
or expired) — each with provenance.

**The model in five sentences.** A prophecy is a proposal, never an
act: nothing in this crate executes anything, and fulfilment is a
record that a gated task ran, taken by the host system. Every
prophecy carries provenance — the observation hash, the log
sequence number, and the moment the evidence was captured — so any
record audits back to the state it was read from. A prophecy is
produced by a `reader` (deterministic code), a `model`, or a
`human`, and the store rejects `requires_human: false` from anyone
but a reader-produced `Finding`. Resolution is terminal and
one-way: `Fulfilled` and `Dismissed` are ends, and an expired
prophecy can no longer be resolved at all — the sweep moves it to
`Expired`. The store is append-only from the producing side, durable
as one JSONL file (mode 0600, malformed lines quarantined to
`prophecies.corrupt` rather than dropped), with model-origin records
capped per hour and reader findings coalesced into summaries, never
dropped.

## State diagram

```
                   ┌─────────────────────────────┐
                   │          Proposed           │
                   │  (open; appended with       │
                   │   provenance; expires_at)    │
                   └──────┬───────┬───────┬──────┘
                          │       │       │
        resolve(Fulfilled)│       │       │ resolve(Dismissed)
        via the host's    │       │       │
        gated path        │       │       └──────────────┐
                          ▼       │       ▼              │
                  ┌───────────┐   │  ┌────────────┐      │
                  │ Fulfilled │   │  │ Dismissed  │      │
                  └───────────┘   │  └────────────┘      │
                                  │                      ▼
                     expiry sweep│              ┌────────────┐
                                  └─────────────▶ │  Expired   │
                                                 └────────────┘
   All three are Fulfilment states: terminal, never re-resolved.
```

## Example

A reader-produced finding that validates against
`schema/prophecy.schema.json`:

```json
{
  "id": "0b8c1f5e-9d3a-4c71-9f2b-6a4e8d7c1a20",
  "kind": {
    "kind": "finding",
    "source": {
      "id": "watch-1",
      "row_key": "sender=a@b.example subject=hello received=2026-10-09T09:15:00Z",
      "row_hash": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    },
    "payload": { "unread": 3 }
  },
  "producer": "reader",
  "provenance": {
    "observation_hash": "67d1659bfe71b89d50b45a4ad1a9e5b9",
    "event_log_seq": 42,
    "captured_at": "2026-10-09T09:15:02Z"
  },
  "requires_human": false,
  "created_at": "2026-10-09T09:15:02Z",
  "expires_at": "2026-10-10T09:15:02Z"
}
```

Validate the record with the store (`append` runs `validate`), or
against the JSON Schema directly — regenerate after any type change:

```sh
cargo run --bin gen-schema > schema/prophecy.schema.json
```

`tests/schema_up_to_date.rs` fails with a diff when the checked-in
schema is stale.

## Embedding

The crate is dependency-isolated (no host-system crates, no domain
vocabulary) and stable for embedding: `Prophecy`, `Kind`,
`Producer`, `Source`, `Provenance`, `Fulfilment`, `State` are the
public surface; `ProphecyStore` is the reference store. The wire
contract is `schema/prophecy.schema.json` (Draft-07).

License: Apache-2.0, holder Suo-commerce.
