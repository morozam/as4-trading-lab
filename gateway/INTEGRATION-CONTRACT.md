# Holodeck REST adapter

These notes describe the adapter used by this app.
Use Holodeck B2B **8.1.1** with REST backend **4.0.0**.
For normal setup, use [the setup guide](../docs/setup.md).

## Included public files

| File and download | Install in | SHA-256 |
|---|---|---|
| [rest-backend-4.0.0.jar](https://github.com/holodeck-b2b/rest-backend/releases/download/v4.0.0/rest-backend-4.0.0.jar) | `lib/` | `808795325a56e20f7aedda9057c5e2e9ad7c787afcab5bbaf71147fab86325bc` |
| [hb2b-rest-backend.aar](https://github.com/holodeck-b2b/rest-backend/releases/download/v4.0.0/hb2b-rest-backend.aar) | `repository/services/` | `369c7bcb5063900da6c0a4cb4347cc3d0b12d95408a7e90ddc8e095b117fe394` |
| [rest-empty-error-2.0.0.mar](https://github.com/holodeck-b2b/axis2-rest-status-only-error/releases/download/2.0.0/rest-empty-error-2.0.0.mar) | `repository/modules/` | `7e9f1ca6c4a24cc44573a405892b6ee165c7c19578960e42dc4f9a9587f374c3` |

`setup.py` checks these files, installs the lab Java extension, creates the
P-Modes and sets the callback addresses. Each role gets separate gateway
storage. The monitor and file-submit workers are disabled in the lab.

## Submit XML

The BRP submits to:

```text
POST http://127.0.0.1:19080/holodeckb2b/restbackend/submit
```

The TSO uses port 19081. Send one raw XML body, not a multipart body.
The app supplies these headers:

| Header | Purpose |
|---|---|
| `Content-Type: application/xml` | Body type |
| `X-HolodeckB2B-PModeId` | `mvp-forward` or `mvp-reverse` |
| `X-HolodeckB2B-MessageId` | New unique AS4 message ID |
| `X-HolodeckB2B-ConversationId` | Link messages in an exchange |
| `X-HolodeckB2B-Containment: ATTACHMENT` | Carry XML as an AS4 attachment |
| `X-HolodeckB2B-RefToMessageId` | Original message ID, when replying |

The P-Mode supplies the sender, receiver, service and action.
The adapter creates an attachment ID if no `Content-Id` is supplied.
A successful REST submission is HTTP 202 with an empty body.
This means the gateway accepted the send; it does not mean the peer received it.
The app chooses its own AS4 ID because this adapter does not return an assigned
ID in the response body. Never reuse an AS4 message ID for a new message.

## Receive XML and signals

The P-Mode's delivery URL has this shape:

```text
http://127.0.0.1:<app-port>/integrations/<secret-token>
```

The adapter appends one of these paths:

| Path | Body | Meaning |
|---|---|---|
| `/deliver` | Raw XML | A business message arrived |
| `/notify/receipt` | Empty | An AS4 receipt arrived |
| `/notify/error` | Empty | An AS4 error arrived |

The adapter sends `X-HolodeckB2B-*` headers with available message and route
metadata. Header names are case-insensitive.
For a receipt, `MessageId` is the receipt's own ID;
`RefToMessageId` identifies our outgoing message.
For an error, `X-HolodeckB2B-Errors` carries the reported errors.

The app stores the callback before returning HTTP 204.
A business rejection is separate XML, not an AS4 error.
An ACKNOW, NOMRES or PROCON goes through a new outgoing REST submission.
Do not return that XML as the HTTP response to `/deliver`.
The lab uses synchronous delivery and synchronous signed AS4 receipts.

## Local security changes

The Java sources in this folder add a localhost listener and an incoming
payload-encryption check. They do not change vendor JAR files.
See [security](../docs/security.md) for the algorithms and limits,
and [settings](../docs/settings.md) for P-Mode locations.

## Upstream reference

- [REST backend 4.0.0 release](https://github.com/holodeck-b2b/rest-backend/releases/tag/v4.0.0)
- [REST API specification](https://github.com/holodeck-b2b/rest-backend/blob/v4.0.0/api_specification.md)
- [Submission code](https://github.com/holodeck-b2b/rest-backend/blob/v4.0.0/src/main/java/org/holodeckb2b/backend/rest/SubmitOperation.java)
- [Callback code](https://github.com/holodeck-b2b/rest-backend/blob/v4.0.0/src/main/java/org/holodeckb2b/backend/rest/NotifyAndDeliverOperation.java)
