# Set up the app

[Back to the main guide](../README.md)

Use Linux, or Linux inside WSL2 on Windows. Native Windows and macOS have not been tested.
Run all commands from the repository folder unless a step says otherwise.

## 1. Get the tools

You need these tools on your PATH:

| Tool | What it does |
|---|---|
| Rust and Cargo | Build the server |
| C compiler and linker | Build Rust dependencies such as bundled SQLite |
| Node.js and npm | Build the web pages |
| Python 3 | Run the setup and start scripts |
| Java 17 JDK | Run Holodeck and build the small Java adapter |
| `xmllint` | Check XML against the Edig@s schemas |
| `openssl` and `keytool` | Read certificates in the app |
| Git, curl and unzip | Download the code and Holodeck |

The app was tested with Rust 1.99.1, Node 26.12.0, npm 11.20.0,
Python 3.15.0 and Java 17. Use the lockfiles in this repo.
On Debian or Ubuntu, the C build tools come from `build-essential`.
`xmllint` comes from `libxml2-utils`.
`keytool` comes with the Java JDK.

## 2. Download Holodeck

Use a clean **Holodeck B2B 8.1.1** distribution.
The repo includes the REST adapter, but not the full Holodeck download.

```bash
mkdir -p .dependencies
curl -fL https://github.com/holodeck-b2b/Holodeck-B2B/releases/download/v8.1.1/holodeckb2b-distribution-8.1.1.zip -o .dependencies/holodeck.zip
printf '%s\n' 'a63355fc02dab653102f47769ecf877d6619779f9dfca6bff3539b42b3fabc02  .dependencies/holodeck.zip' | sha256sum -c -
unzip .dependencies/holodeck.zip -d .dependencies
```

Find the unpacked folder that contains `lib`, `bin`, `conf` and `examples`.
Use its full path in the next step.

## 3. Set two paths

Replace the example paths with your own paths:

```bash
export LAB_HOLODECK_SOURCE=/full/path/to/holodeckb2b-8.1.1
export LAB_JAVA_HOME=/full/path/to/java17
export JAVA_HOME="$LAB_JAVA_HOME"
export PATH="$JAVA_HOME/bin:$PATH"
```

Keep these settings in the terminal when you run the app.
`LAB_JAVA_HOME` must point to the JDK folder, not its `bin` folder.
You can check it with:

```bash
"$LAB_JAVA_HOME/bin/java" -version
javac -version
```

If Holodeck and Java are already in sibling folders named
`holodeckb2b-8.1.1` and `java17`, these are the default paths.

## 4. Build and start

```bash
./run.sh build
./run.sh
```

The first build downloads Rust and Node packages. Allow several minutes.
Startup makes two separate test nodes under `runtime/`.
It copies the public Holodeck example keys. It does not copy any existing
company keys, message history or gateway database.

Open the [login addresses](../README.md#open-the-app).
Use the [user guide](user-guide.md) for your first test.

## After a code update

Keep your two path settings from step 3, then run:

```bash
./run.sh stop
./run.sh build
./run.sh
```

This keeps your saved messages. The setup script also updates the known lab
P-Modes with the lab encryption settings.
