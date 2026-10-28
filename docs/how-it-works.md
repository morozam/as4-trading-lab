# How the app works

[Back to the main guide](../README.md)

## Two separate sides

```text
BRP browser → BRP Rust app → BRP Holodeck
                                  ↓ AS4
TSO browser ← TSO Rust app ← TSO Holodeck
```

Replies go back through the same path in the other direction.
The apps never pass business XML straight to each other.
Holodeck handles AS4 signatures, encryption, receipts and transport retries.
The Rust app handles gas documents, process state and user actions.

Each side has its own SQLite database. A send first saves the XML and a queue
entry. A background worker then gives it to the local Holodeck REST API.
Holodeck delivers incoming XML to the receiving app through a private callback.
The app saves it before returning success.

## Main parts

| Part | Where | Job |
|---|---|---|
| Rust server | `src/` | API, users, message state and XML |
| React + TypeScript | `client/` | Browser pages |
| SQLite | Local `runtime/` | Saved drafts, messages and process history |
| Edig@s 6.1 XSDs | `schemas/` | XML structure checks |
| Holodeck 8.1.1 | External download, copied into `runtime/` | AS4 exchange |
| Holodeck REST adapter 4.0.0 | `gateway/assets/` | Submit XML and deliver callbacks |
| Small Java extensions | `gateway/*.java` | Local-only listener and required payload encryption |
| Python scripts | Repo root | Setup, start, stop and test-kit export |

## Find the code

| File | What it does |
|---|---|
| `src/edigas.rs` | Generate, parse and validate gas XML |
| `src/app.rs` | Database, users and business processes |
| `src/transport.rs` | Queue worker and Holodeck submission |
| `src/web.rs` | HTTP routes and session checks |
| `src/inventory.rs` | Read certificates and P-Modes |
| `client/src/` | Workspace, editors, process steps and message pages |
| `setup.py` | Create the two isolated local nodes |
| `run.py` | Build and control the lab processes |

## Important choices

- Previewing freezes the XML and records its hash, a short fingerprint of its bytes.
- Sending must use that preview's hash. Editing requires a new preview.
- Each send action has a unique retry key (`idempotency_key`). Repeating the same action does not make a second send.
- Each AS4 message has its own ID. A business document has a separate ID and version.
- New versions keep the older XML and events. They do not rewrite history.
- A gateway submission, an AS4 receipt and a business result have separate states.
- If a send's result is uncertain, the app does not blindly create another message.
- Incoming result profiles keep their returned amounts and codes, including warnings or unknown results.

## What the validator does

The app chooses a known local schema for the message family.
It does not download a schema named by the incoming XML.
It uses `xmllint` for XSD checks and Rust for additional lab rules.
The saved checksums identify the supplied Edig@s 6.1 schema files.

The generator and lab rules use test identities from `src/edigas.rs`.
Changing only a Holodeck route will not make the app suitable for real GTS use.
See [GTS readiness](gts-readiness.md).
