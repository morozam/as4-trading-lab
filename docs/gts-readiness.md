# What remains before the GTS check

[Back to the main guide](../README.md)

This repo is a local test app. GTS has not approved it.
The TSO user is a simulator, not GTS.

The intended business scope is TTF trading under licence C:
nomination tests, trade programme tests and within-day balancing notices.
There are no physical entry/exit or capacity workflows in this app.

## Business setup

- Complete GTS onboarding and get the correct company EIC and portfolio codes.
- Agree the test cases and test counterparties with GTS Nomination Support.
- Confirm who nominates each exchange trade. The app has no ICE or EEX connection.
- Agree how OTC trades and corrections will be sent and checked.
- Check that returned hourly profiles match the intended trades.
- Agree how to handle rejection, warnings, missing replies and late messages.

The lab has NOMINT → ACKNOW → NOMRES, PRODOC → ACKNOW → PROCON,
and incoming MARSIT. This does not replace the official communication check.

## Server and AS4 setup

- A reliable server, stable public IP, subdomain and reachable AS4 endpoint.
- HTTPS with a suitable domain certificate.
- A company AS4 signing/decryption certificate from an accepted CA.
- The current GTS public certificates and their trusted CA chain.
- Separate acceptance and production settings and data.
- Both completed GTS communication sheets.
- Matching AS4 identities, routes, algorithms, receipts and profile version.
- Private access to the app, Holodeck REST API and callback routes.
- Backups, monitoring and a plan to replace expiring certificates.

BRP and TSO are two roles in our lab. They are **not** the acceptance and
production environments. Each real environment needs its own setup.

## Code still needed

1. Replace hard-coded test identities and lab-only validation rules with managed company and partner settings.
2. Add the agreed real GTS P-Modes, certificates and endpoints.
3. Add production access controls, HTTPS deployment and secure secret storage.
4. Add operational alerts for failed sends, missing ACKNOW, missing results and expiring certificates.
5. Test restart recovery, backups, corrections and the agreed GTS timing rules.
6. Run the real acceptance tests with GTS, then the agreed production check.

Do not point the current lab at GTS by changing only an endpoint.
Its IDs, party roles and test replies were built for local tests.
The lab's RSA-OAEP settings also do not establish compliance with every newer
ENTSOG profile. Agree the actual profile with GTS before configuring it.

## Documents to use

Use the current documents from [GTS electronic communication](https://www.gasunietransportservices.nl/en/shipper-trader/become-a-shipper-or-trader-with-gts/electronic-communication)
and the current [Edig@s documents](https://www.edigas.org/).
Keep the version and date of the documents used for the real check.

Some older checklists and newer 6.1 guides use different message names or
timings. Confirm these with GTS before coding deadlines or test expectations.
Our local tests do not resolve those differences.
