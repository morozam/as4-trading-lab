# API guide

[Back to the main guide](../README.md)

The API is on the same address as each web app.
Use the BRP API for BRP actions and the TSO API for TSO actions.
JSON requests use `Content-Type: application/json`.
The exact request types are in `src/edigas.rs` and `src/web.rs`.

## Login

Send `POST /api/login` with:

```json
{"username":"brp","password":"local-brp"}
```

Keep the returned session cookie. The JSON response also contains `csrf`.
For later actions that change data, send that value in `X-CSRF-Token`.
If you send an `Origin` header, it must match the app address.
`GET /api/session` reads the current session. `POST /api/logout` ends it.

Most routes need a session. `/api/info` is public.
Callback routes use their private token instead of a browser session.

## Read data

| Method and path | Result |
|---|---|
| `GET /api/info` | Role, environment and other-side link |
| `GET /api/calendar?day=YYYY-MM-DD` | Actual hourly intervals for the gas day |
| `GET /api/state` | Drafts, processes, messages and events |
| `GET /api/messages/{id}/details` | One message and its related details |
| `GET /api/messages/{id}/xml` | Exact XML as text |
| `GET /api/messages/{id}/xml?download=1` | Exact XML as a download |
| `GET /api/gateway/inventory` | Local certificate and P-Mode readout |

Use the app's message ID in the message paths.
A business document ID or AS4 ID is a different value.

## Submit a nomination or programme

1. `POST /api/drafts` creates a draft.
2. `PUT /api/drafts/{id}` changes it with the same input shape.
3. `POST /api/drafts/{id}/preview` generates and checks XML.
4. `POST /api/drafts/{id}/send` sends that preview.

Draft input fields:

| Field | Value |
|---|---|
| `kind` | `NOMINT` or `PRODOC` |
| `gas_day` | `YYYY-MM-DD` |
| `counterparty` | `GSLABCOUNTER` for the lab nomination |
| `direction` | `buy` or `sell` |
| `quantities` | One integer kWh/h amount for each interval from `/api/calendar` |
| `document_id` | `null` for a new document |
| `version` | `1` for a new document |

PRODOC uses the reserved zero trade programme. Send all-zero quantities.
The preview response includes XML, validation results and `hash`.
Do not send a failed preview.

Send input:

```json
{"hash":"value-from-preview","idempotency_key":"new-unique-action-id"}
```

Keep the same retry key if retrying the same action.
`POST /api/processes/{id}/revise` creates a new draft version from a process.
Use its returned ID and version instead of making up a new version yourself.

## TSO replies

| Method and path | Input |
|---|---|
| `POST /api/processes/{id}/ack` | `accepted`, `reason`, `idempotency_key` |
| `PUT /api/fixtures/{id}` | `quantities`: hourly integer array, or `null` for no counter-nomination |
| `POST /api/processes/{id}/result` | `idempotency_key` |
| `POST /api/programmes/default` | `gas_day`, `idempotency_key` |

Use the TSO's process ID. Accept before sending a result.
NOMINT needs a test fixture before NOMRES. PRODOC does not.

## TSO balancing allocation

`POST /api/balancing/preview` accepts:

- `gas_day`, `document_code` (`ANZ` or `AVI`), `document_id` and `version`.
- `transactions`: each has `identification` and `periods`.
- Each period has `start`, `end`, `direction`, `quantity` and `price`.

Use UTC timestamps for period boundaries.
Directions are `debit` or `credit`. Quantity is an integer in kWh/h.
Price is a decimal string in EUR/kWh, for example `"0.035000"`.
Use a lab transaction ID such as `TR000001`, or `NO_TRANS` for no transaction.

`POST /api/balancing/send` takes `preview_id`, `hash` and `idempotency_key`
from the successful preview and your action.

## Validate without sending

`POST /api/validate` takes `{"xml":"the XML text"}`.
It returns the validation report. It creates no business send.

## Private Holodeck callbacks

| Path | What arrives |
|---|---|
| `POST /integrations/{token}/deliver` | Raw XML and Holodeck headers |
| `POST /integrations/{token}/notify/receipt` | AS4 receipt metadata |
| `POST /integrations/{token}/notify/error` | AS4 error metadata |

Setup creates the secret token locally. Do not put a real token in Git.
A successful callback returns HTTP 204. A business reply is sent separately;
it is not returned in the callback's HTTP response.
For a receipt, `RefToMessageId` identifies the outgoing message it confirms.
See the [Holodeck adapter notes](../gateway/INTEGRATION-CONTRACT.md).

## Errors

App errors return a non-success HTTP code and JSON such as:

```json
{"error":"Choose a gas day"}
```

Do not treat a failed or uncertain HTTP send as permission to send a new copy.
Read the process and transport state first.
