# Certificates and security

[Back to the main guide](../README.md)

## Two different jobs

| Certificate | Job |
|---|---|
| Website / HTTPS certificate | Proves the server's domain and protects the HTTP connection |
| AS4 message certificate | Signs messages and lets the owner decrypt incoming messages |

Both can be X.509 certificates. They do different jobs.
Having an HTTPS certificate does not by itself make it suitable for AS4.
A representative's personal signing key is not automatically a company AS4 key.
For GTS, get the required company certificate from an accepted CA with the
right key uses. A signature by a CEO alone does not make a certificate CA-trusted.

## What happens to a message

1. The sender signs it with its own private key.
2. The payload is compressed.
3. It is encrypted for the receiver, using the receiver's public certificate.
4. The receiver uses its private key to decrypt it and checks the signature.
5. The receiver returns a signed AS4 receipt.

Keep private keys secret. Share public certificates and their CA chain when
setting up trust with the other party.
A **CA** is the organisation that issues and signs the certificate.

## What the lab uses

The lab uses Holodeck's public example identities, `partya` and `partyb`.
Their private keys and passwords are public test material.
They are useful for proving the flow. They do not prove a company's identity.
The Git repo contains no local runtime keystores or company private keys.
Setup gets the test keystores from the clean Holodeck distribution.

| Setting | Lab value |
|---|---|
| Signature | RSA-SHA256 |
| Payload encryption | AES-128-GCM |
| Encryption key transport | RSA-OAEP, SHA-256 digest, MGF1-SHA256 |
| Payload compression | AS4 gzip |
| Receipt | Signed, returned on the same HTTP exchange |

In the Holodeck P-Mode, look under:

```text
Responder / SecurityConfiguration / Encryption
Leg / UserMessageFlow / PayloadProfile / UseAS4Compression
```

The abstract standard's `PMode[].Security.X509.Encryption.Encrypt` is not a
literal setting name in Holodeck. Holodeck uses the XML elements above.
The `Encryption` section names the receiver's certificate and algorithms.
The receiver's own node also has access to its private key.

The small `RequirePayloadEncryption` Java handler rejects business payloads
that did not successfully decrypt with AES-128-GCM. It runs before business
delivery. AS4 receipt and error signals follow their own rules.
It does not check every rule of every ENTSOG profile.

## App access

Each side has one local account. Login creates a browser session.
Actions that change data also need a CSRF token, to block cross-site requests.
Holodeck calls back through a separate secret URL token.
Keep that token and the config files private.

The lab uses HTTP and test passwords. Message encryption protects the AS4
payload between gateways; it does not protect a browser's HTTP login or local
files. Use this setup only as a lab on a trusted computer or private network.

For real GTS use, follow the [remaining work](gts-readiness.md), including
real certificates, HTTPS and private access to the app and REST interfaces.
