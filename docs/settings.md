# Settings and files

[Back to the main guide](../README.md)

## Main settings

Setup creates these files. They are private local files, excluded from Git:

```text
runtime/brp/config.json
runtime/tso/config.json
```

Stop the app before changing them. Start it again to use the new settings.
Do not paste a whole config file into a chat or issue: it contains a password
and a secret callback token.

| Setting | Meaning |
|---|---|
| `role` | Which app this is: `brp` or `tso` |
| `bind` | `127.0.0.1` for this laptop; `0.0.0.0` for private network tests |
| `port` | Web app port: BRP 8797, TSO 8798 |
| `data_dir` | Folder for this app's database |
| `schema_dir` | Folder with the XML schemas |
| `ui_dir` | Folder with the built web pages |
| `gateway_url` | This side's local Holodeck REST submit address |
| `peer_url` | Browser link to the other side; this does not route AS4 |
| `pmode_id` | Outgoing Holodeck route ID |
| `inbound_pmode_id` | Expected incoming route ID |
| `callback_token` | Secret in the Holodeck-to-app callback address |
| `username`, `password` | The local test login |

Keep the standard ports. The start scripts and tests expect them.
Changing just a port in this file will not update the whole setup.

## Ports

| Purpose | BRP | TSO |
|---|---|---|
| Web app and API | 8797 | 8798 |
| Holodeck AS4 and REST | 19080 | 19081 |

The gateway listeners bind to `127.0.0.1`.
Their AS4 path is `/holodeckb2b/as4`.
Their REST submit path is `/holodeckb2b/restbackend/submit`.
These are different interfaces on the same gateway port.

## Where is the P-Mode?

A **P-Mode** is a set of rules for an AS4 exchange: who sends, where it goes,
which keys it uses, and how it returns a receipt.

Templates live in `gateway/pmodes/`:

| Template | Used by | Route |
|---|---|---|
| `node-a-forward-sender.xml` | BRP | Send to TSO |
| `node-b-forward-receiver.xml` | TSO | Receive from BRP |
| `node-b-reverse-sender.xml` | TSO | Send to BRP |
| `node-a-reverse-receiver.xml` | BRP | Receive from TSO |

The running copies are in:

```text
runtime/brp/gateway/repository/pmodes/
runtime/tso/gateway/repository/pmodes/
```

The forward route is `mvp-forward`. The return route is `mvp-reverse`.
Setup puts the private callback address into each running copy.
It keeps existing lab routes, and reapplies the known lab encryption policy.
Changing a template alone does not replace an existing running copy.
Restart the gateways after a setting change.

The ebMS `.../oneWay` and `.../push` values describe how AS4 works.
They are names for protocol choices, not website addresses to call.

## Data and backup

| Path | What is there |
|---|---|
| `runtime/<role>/app/app.sqlite3` | Processes, XML, drafts, sessions and send queue |
| `runtime/<role>/gateway/db/` | Holodeck's own message state |
| `runtime/<role>/gateway/repository/certs/` | Local test keystores |
| `runtime/<role>/logs/` | Start script logs |
| `runtime/<role>/gateway/logs/` | Holodeck logs |
| `runtime/<role>/security-backup/` | First saved copies of changed security files |
| `runtime/pids.json` | Processes started by this lab |
| `test-output/` | Test reports and screenshots |

To make a consistent backup, stop both roles and copy the whole `runtime/`
folder to private storage. It contains secrets and message data.
The config uses full paths; moving a backup to another location needs those
paths checked before startup.
