# Other software and files

The app runs beside Holodeck and talks to it through REST.
The full Holodeck distribution and Java runtime are downloaded separately.
They are not stored in this Git repo.

| Component | Version used | Source / licence |
|---|---|---|
| Holodeck B2B | 8.1.1 | [Source](https://github.com/holodeck-b2b/Holodeck-B2B) · GPL-3.0; separate interface components have their own notices |
| Holodeck REST backend | 4.0.0 | [Source](https://github.com/holodeck-b2b/rest-backend) · GPL-3.0 |
| REST error module | 2.0.0 | [Source](https://github.com/holodeck-b2b/axis2-rest-status-only-error) · LGPL-3.0 |
| React and React DOM | See `client/package-lock.json` | [Source](https://github.com/facebook/react) · MIT |
| Lucide icons | See `client/package-lock.json` | [Source](https://github.com/lucide-icons/lucide) · ISC |
| TypeScript | See `client/package-lock.json` | [Source](https://github.com/microsoft/TypeScript) · Apache-2.0 |
| Vite | See `client/package-lock.json` | [Source](https://github.com/vitejs/vite) · MIT |
| Playwright | 1.62.1 | [Source](https://github.com/microsoft/playwright) · Apache-2.0 |
| Edig@s XML schemas | 6.1 supplied bundle | [Edig@s](https://www.edigas.org/) · retain the notices in the XSD files |

Rust package versions are locked in `Cargo.lock`.
Node package versions are locked in `client/package-lock.json`.
Their dependencies keep their own licences.
This file does not choose a licence for the application code.

## Small gateway files included here

`gateway/assets/` includes these public upstream release files:

- `rest-backend-4.0.0.jar`
- `hb2b-rest-backend.aar`
- `rest-empty-error-2.0.0.mar`

Their download links and checksums are in the
[gateway notes](gateway/INTEGRATION-CONTRACT.md).
Their licence texts are in [gateway/licenses](gateway/licenses/).
The local `lab-loopback-transport.jar` is built from the two Java source files
in `gateway/`. Setup rebuilds it when needed.

The example keystores come from Holodeck's `examples/certs/` folder during
setup. They are public test material. Do not use them as company credentials.
Keep the Holodeck and Java licence files with any exported test kit.

The schema checksums are in `schemas/SHA256SUMS`.
Test XML under `tests/fixtures/` uses lab data.
