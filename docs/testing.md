# Run the checks

[Back to the main guide](../README.md)

Use test data. The end-to-end checks create real messages in your local lab
and keep them in its history. Keep the default lab logins for these checks.

## Code and XML checks

From the repo folder:

```bash
CARGO_HOME="$PWD/.cargo" cargo test --locked
python3 tests/test_gateway_security.py
```

For the web app:

```bash
cd client
npm ci
npm test
npm run build
```

Go back to the repo folder for the next steps.
The Rust checks need `xmllint`, `openssl` and Java's `keytool` on PATH.
They cover XML structure, message parsing, process state, login and access rules.
The Python check covers gateway security setup and safe repeat runs.

## Real messages through Holodeck

Start both roles first:

```bash
./run.sh
python3 tests/http_roundtrip.py
python3 tests/as4_encryption.py
```

Run these checks one at a time.
The first sends nominations, programme messages and balancing messages through
the actual gateways. It also tests rejection, matching and new versions.

The encryption test captures test AS4 requests on localhost. It checks that:

- The wire payload is encrypted and the received XML is unchanged.
- Normal messages receive a signed receipt.
- Plaintext and changed ciphertext are rejected before business delivery.
- Both directions behave this way.

It uses temporary outgoing test routes and removes them afterwards.
Expected rejection errors remain in the local gateway logs.
Reports go to `test-output/`; they are excluded from Git.

## Browser checks

The repo includes Playwright as a web-app development dependency.
Install its test browser once:

```bash
cd client
npx playwright install chromium
```

Some Linux systems also need browser system libraries. Playwright lists these
if the browser cannot start. Then, from the repo folder, with both roles running:

```bash
node tests/browser.mjs
node tests/navigation_smoke.mjs
```

Run the main browser check first. The navigation check expects a saved NOMINT.
The checks cover the three processes, new versions, XML highlights, downloads,
message pages, certificate pages and a small phone-sized view.
Screenshots are saved in `test-output/`.

To use an already installed compatible Chromium, set `LAB_CHROMIUM` to its
executable path. To test the main browser flow through your laptop's network
address, set `LAB_HOST` to that IP.

Passing these checks proves the tested lab behaviour. GTS must still test and
accept the real connection and business messages.
