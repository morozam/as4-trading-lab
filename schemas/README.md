# Edig@s 6.1 schemas

These XSD files were copied unchanged from the supplied Edig@s 6.1 schema
bundle on 27 October 2026. `SHA256SUMS` records their checksums.
Keep the original notices inside the files.

The bundle here includes the nomination, response, general, market situation
and shared code-list schemas needed by the lab.
The app chooses a known local schema. It does not download the schema named
by an incoming document.

An XSD checks XML structure and data types. Passing it does not prove that a
trade is correct or that GTS will accept a message.
See [GTS readiness](../docs/gts-readiness.md) for the remaining work.
