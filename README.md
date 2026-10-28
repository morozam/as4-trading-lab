# AS4 Trading Lab

A small app for testing gas messages between a trading company and a TSO.

One person uses the **BRP** side. BRP means the trading company.
The other person uses the **TSO** side. Here, the TSO is a test version of GTS.

Each side has its own app, database and Holodeck gateway.
The messages really travel through Holodeck using AS4.
The company names, keys and trades are test data.

**Edig@s XML** is the gas message format. **AS4** is how the message is sent
securely. **Holodeck** is the software that handles that AS4 delivery.

## Start here

| What you want to do | Read this |
|---|---|
| Install and start the app | [Setup](docs/setup.md) |
| Send a nomination or programme | [User guide](docs/user-guide.md) |
| Let a colleague use the app | [Run the app](docs/running.md) |
| Find settings, P-Modes and logs | [Settings and files](docs/settings.md) |
| Understand certificates and encryption | [Security](docs/security.md) |
| Run the checks | [Testing](docs/testing.md) |
| Understand the code or API | [How it works](docs/how-it-works.md) · [API](docs/api.md) |
| Prepare for the real GTS check | [GTS readiness](docs/gts-readiness.md) |

## Open the app

After setup, run:

```bash
./run.sh
```

| Side | Address | Username | Test password |
|---|---|---|---|
| Trading company | http://127.0.0.1:8797 | `brp` | `local-brp` |
| TSO simulator | http://127.0.0.1:8798 | `tso` | `local-tso` |

These passwords are only for the lab.
A fresh checkout starts both apps on localhost.
Use the [network steps](docs/running.md) to give a colleague access.

## What you can test

```text
Nomination:       BRP sends NOMINT → TSO sends ACKNOW → TSO sends NOMRES
Trade programme:  BRP sends PRODOC → TSO sends ACKNOW → TSO sends PROCON
Balancing:        TSO sends MARSIT → BRP reads the allocation
```

You can also reject a request, send a new version, test a mismatch,
and send a default zero programme.

The app checks XML against the supplied Edig@s 6.1 schemas.
You can open the exact XML for every message.
The process page shows each step and the messages behind it.

AS4 business payloads are signed, compressed and encrypted in both directions.
The gateways return signed AS4 receipts.

## What is still a lab

This app uses made-up EICs, portfolio codes and counterparties.
It does not connect to GTS, ICE or EEX.
The TSO user controls the test replies.
The app does not calculate your live trading position or official POS.

The example certificates are public test keys from Holodeck.
The gateways use local HTTP. HTTPS and real certificates still need work.
Passing our tests does not mean GTS has approved the system.
See [GTS readiness](docs/gts-readiness.md) for the remaining steps.

## Save and stop

```bash
./run.sh status
./run.sh stop
```

Stopping keeps the message history. Closing the browser does not stop the app.
All local data is under `runtime/`. It is excluded from Git.

## Other software

The app uses Rust, React, SQLite, Holodeck B2B and Edig@s schemas.
See [Third-party software](THIRD_PARTY.md) for sources and licence notes.
