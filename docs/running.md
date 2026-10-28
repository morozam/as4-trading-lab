# Run the app with a colleague

[Back to the main guide](../README.md)

## One laptop, two people

The easiest test is to run both sides on one laptop.
Your colleague opens the TSO page from another device on the same private network.

1. Stop the app with `./run.sh stop`.
2. Edit `runtime/brp/config.json` and `runtime/tso/config.json`.
3. Set `bind` to `0.0.0.0` in both files.
4. Set `peer_url` to the other app's network address.
5. Run `./run.sh` again.

For example, if the laptop's network IP is `192.168.1.50`:

| File | `bind` | `peer_url` |
|---|---|---|
| BRP config | `0.0.0.0` | `http://192.168.1.50:8798` |
| TSO config | `0.0.0.0` | `http://192.168.1.50:8797` |

Your colleague opens `http://192.168.1.50:8798`.
Find your actual IP with `hostname -I`; do not type `0.0.0.0` into the browser.
The laptop must stay awake. Both devices must be able to reach each other.
If a firewall blocks the app, allow TCP 8797 and 8798 from your private network.
A guest Wi-Fi network may block device-to-device access.

Leave the AS4 gateway addresses and ports unchanged.
The two Holodeck gateways still talk over localhost.
The lab web pages use HTTP, so use a trusted private network for this test.
The app's private callback routes also use the app ports; never share their tokens.

## Useful commands

```bash
./run.sh status
./run.sh stop
./run.sh start --role brp
./run.sh start --role tso
```

The app keeps running when you close the browser.
Stopping the app keeps the databases and messages.
Do not remove `runtime/` to fix a network problem.

## Watch Holodeck live

Open a second terminal:

```bash
tail -f runtime/brp/logs/gateway.log runtime/tso/logs/gateway.log
```

For the business app logs:

```bash
tail -f runtime/brp/logs/app.log runtime/tso/logs/app.log
```

Holodeck also writes its own logs under `runtime/<role>/gateway/logs/`.
Use Ctrl+C to stop watching a log. This does not stop the app.
Logs may contain message data. Keep them private.

## Two laptops, one role each

This needs an SSH connection between the computers.
Both gateways stay on localhost; SSH carries their traffic.
Do this on a fresh test setup, with no pending sends.

1. Install the app on both computers and run `python3 setup.py` on each.
2. Stop both copies. Run only BRP on the first computer and TSO on the second.
3. From the BRP computer, open the tunnel below to the TSO computer.
4. Change each side's outgoing route with `configure-peer.py`.
5. Start each role and keep the tunnel open during the test.

On the BRP computer, replace the SSH address:

```bash
ssh -N -o ExitOnForwardFailure=yes -L 29081:127.0.0.1:19081 -L 28798:127.0.0.1:8798 -R 29080:127.0.0.1:19080 -R 28797:127.0.0.1:8797 colleague@tso-laptop
```

In another terminal on the BRP computer:

```bash
python3 configure-peer.py --role brp --peer-as4 http://127.0.0.1:29081/holodeckb2b/as4 --peer-ui http://127.0.0.1:28798
./run.sh start --role brp
```

On the TSO computer:

```bash
python3 configure-peer.py --role tso --peer-as4 http://127.0.0.1:29080/holodeckb2b/as4 --peer-ui http://127.0.0.1:28797
./run.sh start --role tso
```

Use the localhost app address on each computer.
The route helper rejects public URLs and refuses to change routes with unresolved sends.

## Make a test kit

On a Linux computer with the app built and the two setup paths set:

```bash
python3 export-colleague.py
```

It creates `dist/nomination-lab-colleague-linux.tar.gz`.
The kit includes the app build, docs, Java and a clean Holodeck distribution.
It excludes local databases, sessions, logs and company keys.
It uses public example keys.

On a compatible Linux or WSL2 computer, unpack it, enter
`nomination-lab-kit/nomination-lab`, then run `./run.sh`.
The kit needs Python, `xmllint` and `openssl`; it still needs a compatible Linux
system (glibc 2.34 or newer). Use a matching CPU architecture.
