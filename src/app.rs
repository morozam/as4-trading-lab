use crate::edigas::{self, BusinessDocument, DraftInput, MatchingResult, ValidationReport};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct Error {
    pub status: u16,
    pub message: String,
}
impl Error {
    pub fn bad(m: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: m.into(),
        }
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self {
            status: 409,
            message: m.into(),
        }
    }
    pub fn forbidden() -> Self {
        Self {
            status: 403,
            message: "This action belongs to the other role".into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Error {}
impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::bad(s)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self {
            status: 500,
            message: format!("Database error: {e}"),
        }
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::bad(format!("Invalid stored or supplied JSON: {e}"))
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self {
            status: 500,
            message: format!("Storage error: {e}"),
        }
    }
}

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn hash(bytes: &str) -> String {
    hex::encode(Sha256::digest(bytes.as_bytes()))
}
pub fn id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub role: String,
    pub bind: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub schema_dir: PathBuf,
    pub ui_dir: PathBuf,
    pub gateway_url: String,
    pub peer_url: String,
    pub pmode_id: String,
    pub inbound_pmode_id: String,
    pub callback_token: String,
    pub username: String,
    pub password: String,
}
impl Config {
    pub fn local(role: &str, data_dir: PathBuf) -> Self {
        let brp = role == "brp";
        Self {
            role: role.into(),
            bind: "127.0.0.1".into(),
            port: if brp { 8797 } else { 8798 },
            data_dir,
            schema_dir: PathBuf::from("schemas"),
            ui_dir: PathBuf::from("client/dist"),
            gateway_url: format!(
                "http://127.0.0.1:{}/holodeckb2b/restbackend/submit",
                if brp { 19080 } else { 19081 }
            ),
            peer_url: format!("http://127.0.0.1:{}", if brp { 8798 } else { 8797 }),
            pmode_id: if brp {
                "lab-brp-to-tso-send"
            } else {
                "lab-tso-to-brp-send"
            }
            .into(),
            inbound_pmode_id: if brp {
                "lab-tso-to-brp-receive"
            } else {
                "lab-brp-to-tso-receive"
            }
            .into(),
            callback_token: id().replace('-', ""),
            username: role.into(),
            password: format!("local-{role}"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    pub input: DraftInput,
    pub updated_at: String,
    pub review_hash: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub at: String,
    pub message_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Process {
    pub id: String,
    pub document_id: String,
    pub version: u32,
    pub kind: String,
    pub gas_day: String,
    pub counterparty: String,
    pub direction: String,
    pub quantities: Vec<i64>,
    pub confirmed: Vec<i64>,
    pub counterparty_quantities: Vec<i64>,
    pub matching_statuses: Vec<String>,
    pub stage: String,
    pub handoff_status: String,
    pub transport_status: String,
    pub ack_status: String,
    pub validation: ValidationReport,
    pub events: Vec<Event>,
    pub created_at: String,
    pub source_message_id: String,
    #[serde(default)]
    pub balancing: Option<edigas::BalancingDetail>,
    #[serde(default)]
    pub programme_origin: Option<String>,
    #[serde(default)]
    pub programme_portfolio: Option<String>,
    #[serde(default)]
    pub nomination_profiles: Vec<edigas::NominationProfile>,
    #[serde(default)]
    pub programme_profiles: Vec<edigas::ProgrammeProfile>,
    #[serde(default)]
    pub confirmation_accepted: Option<bool>,
    #[serde(default)]
    pub confirmation_reasons: Vec<String>,
    #[serde(default)]
    pub confirmation_document_id: Option<String>,
    #[serde(default)]
    pub confirmation_version: Option<u32>,
    #[serde(default)]
    pub confirmation_origin: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub family: String,
    pub code: String,
    pub direction: String,
    pub document_id: String,
    pub version: u32,
    pub process_id: Option<String>,
    pub at: String,
    pub transport_status: String,
    pub hash: String,
    pub xml: String,
    pub validation: ValidationReport,
}
#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    pub xml: String,
    pub hash: String,
    pub validation: ValidationReport,
    pub draft: Draft,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalancingPreview {
    pub id: String,
    pub input: edigas::BalancingInput,
    pub xml: String,
    pub hash: String,
    pub validation: ValidationReport,
}
#[derive(Clone, Debug, Serialize)]
pub struct ConnectionState {
    pub gateway_url: String,
    pub queued: i64,
    pub unknown: i64,
    pub last_callback: Option<String>,
    pub gateway_reachable: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub role: String,
    pub user: String,
    pub environment: String,
    pub peer_url: String,
    pub connection: ConnectionState,
    pub drafts: Vec<Draft>,
    pub processes: Vec<Process>,
    pub messages: Vec<Message>,
    pub fixtures: std::collections::HashMap<String, Option<Vec<i64>>>,
}

pub struct App {
    pub config: Config,
    pub db: Mutex<Connection>,
    pub gateway_reachable: AtomicBool,
}

fn event(
    p: &mut Process,
    kind: &str,
    title: &str,
    detail: impl Into<String>,
    message: Option<&str>,
) {
    p.events.push(Event {
        id: id(),
        kind: kind.into(),
        title: title.into(),
        detail: detail.into(),
        at: now(),
        message_id: message.map(String::from),
    });
}
pub fn process_get(db: &Connection, key: &str) -> Result<Process> {
    let s: Option<String> = db
        .query_row("SELECT data FROM processes WHERE id=?", [key], |r| r.get(0))
        .optional()?;
    match s {
        Some(s) => Ok(serde_json::from_str(&s)?),
        None => Err(Error {
            status: 404,
            message: "Process not found".into(),
        }),
    }
}
pub fn process_save(db: &Connection, p: &Process) -> Result<()> {
    db.execute(
        "UPDATE processes SET data=? WHERE id=?",
        params![serde_json::to_string(p)?, p.id],
    )?;
    Ok(())
}
fn draft_get(db: &Connection, key: &str) -> Result<Draft> {
    let s: Option<String> = db
        .query_row(
            "SELECT data FROM drafts WHERE id=? AND sent_process IS NULL",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    s.map(|s| serde_json::from_str(&s).map_err(Error::from))
        .unwrap_or_else(|| {
            Err(Error {
                status: 404,
                message: "Draft not found or already sent".into(),
            })
        })
}
fn process_insert(db: &Connection, p: &Process, issuer: &str) -> Result<()> {
    db.execute("INSERT INTO processes(id,document_id,version,kind,issuer,data,result_version) VALUES(?,?,?,?,?,?,0)",params![p.id,p.document_id,p.version,p.kind,issuer,serde_json::to_string(p)?])?;
    Ok(())
}
fn message_get(db: &Connection, key: &str) -> Result<Message> {
    let s: String = db.query_row("SELECT data FROM messages WHERE id=?", [key], |r| r.get(0))?;
    Ok(serde_json::from_str(&s)?)
}
fn message_insert(db: &Connection, m: &Message) -> Result<()> {
    db.execute("INSERT INTO messages(id,direction,document_id,version,family,hash,data,created_at) VALUES(?,?,?,?,?,?,?,?)",params![m.id,m.direction,m.document_id,m.version,m.family,m.hash,serde_json::to_string(m)?,m.at])?;
    Ok(())
}
fn command_get(
    db: &Connection,
    key: &str,
    scope: &str,
    fingerprint: &str,
) -> Result<Option<Value>> {
    if key.is_empty() || key.len() > 200 {
        return Err(Error::bad("A bounded idempotency key is required"));
    }
    let row: Option<(String, String, String)> = db
        .query_row(
            "SELECT scope,fingerprint,response FROM commands WHERE id=?",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    match row {
        None => Ok(None),
        Some((s, f, r)) if s == scope && f == fingerprint => Ok(Some(serde_json::from_str(&r)?)),
        Some(_) => Err(Error::conflict(
            "This idempotency key was used for a different command",
        )),
    }
}
fn command_save(
    db: &Connection,
    key: &str,
    scope: &str,
    fingerprint: &str,
    response: Value,
) -> Result<()> {
    db.execute(
        "INSERT INTO commands(id,scope,fingerprint,response) VALUES(?,?,?,?)",
        params![key, scope, fingerprint, response.to_string()],
    )?;
    Ok(())
}

impl App {
    pub fn open(config: Config) -> Result<Arc<Self>> {
        if !["brp", "tso"].contains(&config.role.as_str()) {
            return Err(Error::bad("Role must be brp or tso"));
        }
        std::fs::create_dir_all(&config.data_dir)?;
        let db = Connection::open(config.data_dir.join("app.sqlite3"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS sessions(token TEXT PRIMARY KEY,csrf TEXT NOT NULL,expires INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS drafts(id TEXT PRIMARY KEY,data TEXT NOT NULL,review_xml TEXT,validation TEXT,sent_process TEXT);
          CREATE TABLE IF NOT EXISTS processes(id TEXT PRIMARY KEY,document_id TEXT NOT NULL,version INTEGER NOT NULL,kind TEXT NOT NULL,issuer TEXT NOT NULL,data TEXT NOT NULL,result_version INTEGER NOT NULL,UNIQUE(document_id,version,kind,issuer));
          CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,direction TEXT NOT NULL,document_id TEXT NOT NULL,version INTEGER NOT NULL,family TEXT NOT NULL,hash TEXT NOT NULL,data TEXT NOT NULL,created_at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS business_facts(issuer TEXT,document_id TEXT,version INTEGER,code TEXT,hash TEXT NOT NULL,PRIMARY KEY(issuer,document_id,version,code));
          CREATE TABLE IF NOT EXISTS callbacks(id TEXT PRIMARY KEY,hash TEXT NOT NULL,kind TEXT NOT NULL,headers TEXT NOT NULL,body TEXT NOT NULL,at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS fixtures(process_id TEXT PRIMARY KEY,quantities TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS settlements(stream TEXT PRIMARY KEY,quantities TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS commands(id TEXT PRIMARY KEY,scope TEXT NOT NULL,fingerprint TEXT NOT NULL,response TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS balancing_previews(id TEXT PRIMARY KEY,data TEXT NOT NULL,sent_process TEXT);
          CREATE TABLE IF NOT EXISTS outbox(id INTEGER PRIMARY KEY AUTOINCREMENT,message_id TEXT NOT NULL UNIQUE,process_id TEXT NOT NULL,payload TEXT NOT NULL,ref_to TEXT,status TEXT NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,error TEXT,started_at TEXT,available_at INTEGER NOT NULL DEFAULT 0);
          CREATE INDEX IF NOT EXISTS outbox_queue ON outbox(status,available_at);
          CREATE INDEX IF NOT EXISTS message_history ON messages(created_at);")?;
        let existing: Option<String> = db
            .query_row("SELECT value FROM settings WHERE key='role'", [], |r| {
                r.get(0)
            })
            .optional()?;
        if existing.as_deref().is_some_and(|r| r != config.role) {
            return Err(Error::bad("A node database cannot change BRP/TSO role"));
        }
        db.execute(
            "INSERT OR IGNORE INTO settings(key,value) VALUES('role',?)",
            [&config.role],
        )?;
        let password_exists: i64 = db.query_row(
            "SELECT count(*) FROM settings WHERE key='password_hash'",
            [],
            |r| r.get(0),
        )?;
        if password_exists == 0 {
            let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
                .map_err(|e| Error::bad(e.to_string()))?;
            let hashed = Argon2::default()
                .hash_password(config.password.as_bytes(), &salt)
                .map_err(|e| Error::bad(e.to_string()))?
                .to_string();
            db.execute(
                "INSERT INTO settings(key,value) VALUES('password_hash',?)",
                [hashed],
            )?;
        }
        // A crash after marking dispatch is ambiguous; do not automatically resend.
        let interrupted: Vec<String> = {
            let mut q = db.prepare("SELECT process_id FROM outbox WHERE status='dispatching'")?;
            let values = q
                .query_map([], |r| r.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            values
        };
        for key in interrupted {
            let mut p = process_get(&db, &key)?;
            p.handoff_status = "Delivery unknown".into();
            if p.ack_status == "Awaiting" {
                p.stage = "Delivery unknown".into()
            }
            event(&mut p,"handoff","Interrupted handoff","Restart found an in-flight handoff. Investigate gateway evidence before any resend.",None);
            process_save(&db, &p)?;
        }
        db.execute("UPDATE outbox SET status='unknown',error='Interrupted during handoff; investigate' WHERE status='dispatching'",[])?;
        Ok(Arc::new(Self {
            config,
            db: Mutex::new(db),
            gateway_reachable: AtomicBool::new(false),
        }))
    }
    fn role(&self, role: &str) -> Result<()> {
        if self.config.role == role {
            Ok(())
        } else {
            Err(Error::forbidden())
        }
    }
    pub fn login(&self, user: &str, password: &str) -> Result<(String, String)> {
        let db = self.db.lock().unwrap();
        let hashed: String = db.query_row(
            "SELECT value FROM settings WHERE key='password_hash'",
            [],
            |r| r.get(0),
        )?;
        let parsed = PasswordHash::new(&hashed)
            .map_err(|_| Error::bad("Account password data unavailable"))?;
        if user != self.config.username
            || Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_err()
        {
            return Err(Error {
                status: 401,
                message: "Incorrect username or password for this node".into(),
            });
        }
        let token = id();
        let csrf = id();
        db.execute(
            "DELETE FROM sessions WHERE expires < ?",
            [Utc::now().timestamp()],
        )?;
        db.execute(
            "INSERT INTO sessions(token,csrf,expires) VALUES(?,?,?)",
            params![token, csrf, Utc::now().timestamp() + 12 * 3600],
        )?;
        Ok((token, csrf))
    }
    pub fn session(&self, token: &str) -> Result<String> {
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT csrf FROM sessions WHERE token=? AND expires>?",
            params![token, Utc::now().timestamp()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(Error {
            status: 401,
            message: "Please sign in".into(),
        })
    }
    pub fn logout(&self, token: &str) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .execute("DELETE FROM sessions WHERE token=?", [token])?;
        Ok(())
    }
    pub fn create_draft(&self, mut input: DraftInput) -> Result<Draft> {
        self.role("brp")?;
        input.version = 1;
        input.document_id = Some(format!("{}-{}-{}", input.kind, input.gas_day, &id()[..8]));
        self.check_input(&input)?;
        let d = Draft {
            id: id(),
            input,
            updated_at: now(),
            review_hash: None,
        };
        self.db.lock().unwrap().execute(
            "INSERT INTO drafts(id,data) VALUES(?,?)",
            params![d.id, serde_json::to_string(&d)?],
        )?;
        Ok(d)
    }
    fn check_input(&self, input: &DraftInput) -> Result<()> {
        if !["NOMINT", "PRODOC"].contains(&input.kind.as_str()) {
            return Err(Error::bad("Only NOMINT and PRODOC are available"));
        }
        if !["buy", "sell"].contains(&input.direction.as_str()) {
            return Err(Error::bad("Direction must be buy or sell"));
        }
        let hours = edigas::intervals(&input.gas_day)?;
        if input.quantities.len() != hours.len() {
            return Err(Error::bad(format!(
                "Expected {} hourly quantities for this gas day",
                hours.len()
            )));
        }
        if input
            .quantities
            .iter()
            .any(|q| *q < 0 || *q > 1_000_000_000)
        {
            return Err(Error::bad(
                "Quantities must be integers between 0 and 1,000,000,000 kWh/h",
            ));
        }
        if input.kind == "PRODOC" && input.quantities.iter().any(|q| *q != 0) {
            return Err(Error::bad(
                "The pure-trader programme must be zero in every hour",
            ));
        }
        if input.counterparty.is_empty() || input.counterparty.len() > 50 {
            return Err(Error::bad(
                "Counterparty is required (maximum 50 characters)",
            ));
        }
        Ok(())
    }
    pub fn update_draft(&self, key: &str, mut input: DraftInput) -> Result<Draft> {
        self.role("brp")?;
        self.check_input(&input)?;
        let db = self.db.lock().unwrap();
        let old = draft_get(&db, key)?;
        if old.input.kind != input.kind || old.input.gas_day != input.gas_day {
            return Err(Error::bad(
                "Create another draft to change document family or gas day",
            ));
        }
        if old.input.version > 1
            && (old.input.direction != input.direction
                || old.input.counterparty != input.counterparty)
        {
            return Err(Error::bad("This MVP revision keeps the original side and counterparty. Create a separate nomination for a different stream."));
        }
        input.document_id = old.input.document_id;
        input.version = old.input.version;
        let d = Draft {
            id: key.into(),
            input,
            updated_at: now(),
            review_hash: None,
        };
        db.execute(
            "UPDATE drafts SET data=?,review_xml=NULL,validation=NULL WHERE id=?",
            params![serde_json::to_string(&d)?, key],
        )?;
        Ok(d)
    }
    pub fn preview(&self, key: &str) -> Result<Preview> {
        self.role("brp")?;
        let db = self.db.lock().unwrap();
        let mut draft = draft_get(&db, key)?;
        self.check_input(&draft.input)?;
        let xml = edigas::generate_submission(&draft.input)?;
        let validation = edigas::validate_xml(&xml, &self.config.schema_dir);
        let digest = hash(&xml);
        draft.review_hash = Some(digest.clone());
        db.execute(
            "UPDATE drafts SET data=?,review_xml=?,validation=? WHERE id=?",
            params![
                serde_json::to_string(&draft)?,
                xml,
                serde_json::to_string(&validation)?,
                key
            ],
        )?;
        Ok(Preview {
            xml,
            hash: digest,
            validation,
            draft,
        })
    }
    pub fn send_draft(&self, key: &str, review_hash: &str, command: &str) -> Result<String> {
        self.role("brp")?;
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let scope = format!("send:{key}");
        if let Some(v) = command_get(&tx, command, &scope, review_hash)? {
            return Ok(v["process_id"].as_str().unwrap_or_default().into());
        }
        let draft = draft_get(&tx, key)?;
        let reviewed: Option<(String, String)> = tx
            .query_row(
                "SELECT review_xml,validation FROM drafts WHERE id=? AND review_xml IS NOT NULL",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (xml, report) = reviewed
            .ok_or_else(|| Error::conflict("Validate and review this draft before sending"))?;
        if draft.review_hash.as_deref() != Some(review_hash) || hash(&xml) != review_hash {
            return Err(Error::conflict(
                "The review is stale. Validate and review again",
            ));
        }
        let validation: ValidationReport = serde_json::from_str(&report)?;
        if !validation.valid {
            return Err(Error::bad("Resolve validation errors before sending"));
        }
        let doc = edigas::parse_document(&xml)?;
        let max: u32 = tx.query_row(
            "SELECT coalesce(max(version),0) FROM processes WHERE document_id=? AND issuer=?",
            params![doc.id, doc.issuer],
            |r| r.get(0),
        )?;
        if doc.version <= max {
            return Err(Error::conflict(
                "A newer or equal version has already been submitted. Prepare a fresh revision",
            ));
        }
        let mut p = new_process(&doc, &validation, "Queued", "");
        event(
            &mut p,
            "submission",
            "Submission queued",
            format!(
                "{} / version {} frozen for Holodeck handoff",
                doc.kind, doc.version
            ),
            None,
        );
        process_insert(&tx, &p, &doc.issuer)?;
        queue_xml(&tx, &mut p, &xml, &validation, None)?;
        tx.execute(
            "UPDATE drafts SET sent_process=? WHERE id=?",
            params![p.id, key],
        )?;
        command_save(
            &tx,
            command,
            &scope,
            review_hash,
            json!({"process_id":p.id}),
        )?;
        tx.commit()?;
        Ok(p.id)
    }
    pub fn revise(&self, key: &str) -> Result<Draft> {
        self.role("brp")?;
        let db = self.db.lock().unwrap();
        let p = process_get(&db, key)?;
        if !matches!(p.kind.as_str(), "NOMINT" | "PRODOC") {
            return Err(Error::bad(
                "Only submitted NOMINT or PRODOC processes can be revised",
            ));
        }
        let max: u32 = db.query_row(
            "SELECT max(version) FROM processes WHERE document_id=?",
            [&p.document_id],
            |r| r.get(0),
        )?;
        let input = DraftInput {
            kind: p.kind,
            gas_day: p.gas_day,
            counterparty: p.counterparty,
            direction: p.direction,
            quantities: p.quantities,
            document_id: Some(p.document_id),
            version: max + 1,
        };
        let d = Draft {
            id: id(),
            input,
            updated_at: now(),
            review_hash: None,
        };
        db.execute(
            "INSERT INTO drafts(id,data) VALUES(?,?)",
            params![d.id, serde_json::to_string(&d)?],
        )?;
        Ok(d)
    }
    pub fn acknowledge(
        &self,
        key: &str,
        accepted: bool,
        reason: &str,
        command: &str,
    ) -> Result<String> {
        self.role("tso")?;
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let fingerprint = hash(&format!("{accepted}:{reason}"));
        let scope = format!("ack:{key}");
        if let Some(v) = command_get(&tx, command, &scope, &fingerprint)? {
            return Ok(v["message_id"].as_str().unwrap_or_default().into());
        }
        let mut p = process_get(&tx, key)?;
        if p.ack_status != "Awaiting" {
            return Err(Error::conflict(
                "An ACKNOW decision already exists for this version",
            ));
        }
        if !p.validation.valid {
            return Err(Error::bad(
                "Invalid XML/profile is quarantined; an ACKNOW is not generated",
            ));
        }
        if !accepted && reason.trim().is_empty() {
            return Err(Error::bad("Give a business rejection reason"));
        }
        let m = message_get(&tx, &p.source_message_id)?;
        let request = edigas::parse_document(&m.xml)?;
        let xml = edigas::generate_ack(&request, accepted, reason)?;
        let validation = edigas::validate_xml(&xml, &self.config.schema_dir);
        ensure_valid(&validation)?;
        p.ack_status = if accepted { "Accepted" } else { "Rejected" }.into();
        p.stage = p.ack_status.clone();
        let ref_to = p.source_message_id.clone();
        let message = queue_xml(&tx, &mut p, &xml, &validation, Some(&ref_to))?;
        event(
            &mut p,
            "business",
            "ACKNOW queued",
            if accepted {
                "Accepted for processing"
            } else {
                reason
            },
            Some(&message),
        );
        process_save(&tx, &p)?;
        command_save(
            &tx,
            command,
            &scope,
            &fingerprint,
            json!({"message_id":message}),
        )?;
        tx.commit()?;
        Ok(message)
    }
    pub fn set_fixture(&self, key: &str, quantities: Option<Vec<i64>>) -> Result<()> {
        self.role("tso")?;
        let db = self.db.lock().unwrap();
        let p = process_get(&db, key)?;
        if p.kind != "NOMINT" {
            return Err(Error::bad("Programmes do not use counterparty fixtures"));
        }
        if p.ack_status != "Accepted" {
            return Err(Error::bad(
                "Accept the request before configuring its matching fixture",
            ));
        }
        if let Some(q) = &quantities {
            if q.len() != p.quantities.len() || q.iter().any(|n| *n < 0 || *n > 1_000_000_000) {
                return Err(Error::bad(
                    "Fixture must cover every hour with bounded nonnegative kWh/h",
                ));
            }
        }
        db.execute("INSERT INTO fixtures(process_id,quantities) VALUES(?,?) ON CONFLICT(process_id) DO UPDATE SET quantities=excluded.quantities",params![key,serde_json::to_string(&quantities)?])?;
        Ok(())
    }
    pub fn send_result(&self, key: &str, command: &str) -> Result<(String, MatchingResult)> {
        self.role("tso")?;
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let scope = format!("result:{key}");
        if let Some(v) = command_get(&tx, command, &scope, "result")? {
            return Ok((
                v["message_id"].as_str().unwrap_or_default().into(),
                serde_json::from_value(v["result"].clone())?,
            ));
        }
        let mut p = process_get(&tx, key)?;
        if p.ack_status != "Accepted" {
            return Err(Error::bad(
                "Accept the request with ACKNOW before producing a result",
            ));
        }
        let newest: u32 = tx.query_row(
            "SELECT max(version) FROM processes WHERE document_id=?",
            [&p.document_id],
            |r| r.get(0),
        )?;
        if p.version < newest {
            return Err(Error::conflict(
                "A later nomination version exists; process that version instead",
            ));
        }
        let message = message_get(&tx, &p.source_message_id)?;
        let mut request = edigas::parse_document(&message.xml)?;
        let fixture: Option<String> = tx
            .query_row(
                "SELECT quantities FROM fixtures WHERE process_id=?",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        let counter: Option<Vec<i64>> = fixture
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .flatten();
        let stream = format!("{}:{}:{}", p.document_id, p.gas_day, p.counterparty);
        let prior: Option<String> = tx
            .query_row(
                "SELECT quantities FROM settlements WHERE stream=?",
                [&stream],
                |r| r.get(0),
            )
            .optional()?;
        let prior: Option<Vec<Option<i64>>> =
            prior.map(|s| serde_json::from_str(&s)).transpose()?;
        let result = if p.kind == "PRODOC" {
            MatchingResult {
                quantities: vec![0; p.quantities.len()],
                statuses: vec!["12G".into(); p.quantities.len()],
            }
        } else {
            edigas::match_nomination(&p.quantities, counter.as_deref(), prior.as_deref())?
        };
        request.counterparty_quantities = counter.clone().unwrap_or_default();
        let response_id = edigas::response_document_id(&request);
        let seq:u32=tx.query_row("SELECT coalesce(max(version),0)+1 FROM messages WHERE direction='out' AND document_id=? AND family=?",params![response_id,if p.kind=="PRODOC"{"PROCON"}else{"NOMRES"}],|r|r.get(0))?;
        let xml = edigas::generate_result(&request, &result, seq)?;
        let validation = edigas::validate_xml(&xml, &self.config.schema_dir);
        ensure_valid(&validation)?;
        p.confirmed = result.quantities.clone();
        p.matching_statuses = result.statuses.clone();
        p.counterparty_quantities = request.counterparty_quantities;
        p.stage = result_stage(&p.kind, &result.statuses);
        let result_document = edigas::parse_document(&xml)?;
        apply_confirmation(&mut p, &result_document, None);
        let ref_to = p.source_message_id.clone();
        let response = queue_xml(&tx, &mut p, &xml, &validation, Some(&ref_to))?;
        let detail = format!(
            "{} / {}",
            if p.kind == "PRODOC" {
                "PROCON"
            } else {
                "NOMRES"
            },
            p.stage
        );
        event(&mut p, "result", "Result queued", detail, Some(&response));
        process_save(&tx, &p)?;
        tx.execute(
            "UPDATE processes SET result_version=? WHERE id=?",
            params![seq, key],
        )?;
        if p.kind == "NOMINT" {
            let settled: Vec<Option<i64>> = result
                .statuses
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    if s == "12G" {
                        Some(result.quantities[i])
                    } else {
                        prior.as_ref().and_then(|q| q[i])
                    }
                })
                .collect();
            tx.execute("INSERT INTO settlements(stream,quantities) VALUES(?,?) ON CONFLICT(stream) DO UPDATE SET quantities=excluded.quantities",params![stream,serde_json::to_string(&settled)?])?;
        }
        command_save(
            &tx,
            command,
            &scope,
            "result",
            json!({"message_id":response,"result":result}),
        )?;
        tx.commit()?;
        Ok((response, result))
    }
    pub fn state(&self) -> Result<State> {
        let db = self.db.lock().unwrap();
        let drafts = query_json::<Draft>(
            &db,
            "SELECT data FROM drafts WHERE sent_process IS NULL ORDER BY rowid DESC LIMIT 100",
        )?;
        let processes = query_json::<Process>(
            &db,
            "SELECT data FROM processes ORDER BY rowid DESC LIMIT 100",
        )?;
        let messages = query_json::<Message>(
            &db,
            "SELECT data FROM messages ORDER BY rowid DESC LIMIT 150",
        )?;
        let mut fixtures = std::collections::HashMap::new();
        let mut q = db.prepare("SELECT process_id,quantities FROM fixtures")?;
        for row in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (k, v) = row?;
            fixtures.insert(k, serde_json::from_str(&v)?);
        }
        let connection = ConnectionState {
            gateway_url: self.config.gateway_url.clone(),
            queued: db.query_row(
                "SELECT count(*) FROM outbox WHERE status IN ('queued','dispatching')",
                [],
                |r| r.get(0),
            )?,
            unknown: db.query_row(
                "SELECT count(*) FROM outbox WHERE status='unknown'",
                [],
                |r| r.get(0),
            )?,
            last_callback: db.query_row("SELECT max(at) FROM callbacks", [], |r| r.get(0))?,
            gateway_reachable: self.gateway_reachable.load(Ordering::Relaxed),
        };
        Ok(State {
            role: self.config.role.clone(),
            user: self.config.username.clone(),
            environment: "LOCAL AS4".into(),
            peer_url: self.config.peer_url.clone(),
            connection,
            drafts,
            processes,
            messages,
            fixtures,
        })
    }
    pub fn xml(&self, key: &str) -> Result<String> {
        Ok(message_get(&self.db.lock().unwrap(), key)?.xml)
    }
    /// Public evidence view. Callback authentication headers and request bodies
    /// are deliberately never copied into this response.
    pub fn message_details(&self, key: &str) -> Result<Value> {
        let db = self.db.lock().unwrap();
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE id=?)",
            [key],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(Error {
                status: 404,
                message: "Message not found".into(),
            });
        }
        let message = message_get(&db, key)?;
        let business = edigas::parse_document(&message.xml).ok();
        let redact = |raw: &str| {
            let mut text = raw.to_string();
            for secret in [&self.config.callback_token, &self.config.password] {
                if !secret.is_empty() {
                    text = text.replace(secret, "[redacted]");
                }
            }
            if let Ok(url) = reqwest::Url::parse(&self.config.gateway_url) {
                if let Some(password) = url.password() {
                    if !password.is_empty() {
                        text = text.replace(password, "[redacted]");
                    }
                }
            }
            text
        };
        let outbox: Option<Value> = db.query_row("SELECT message_id,process_id,ref_to,status,attempts,error,started_at,available_at FROM outbox WHERE message_id=?",[key],|r| {
            let error: Option<String> = r.get(5)?;
            Ok(json!({"message_id":r.get::<_,String>(0)?,"process_id":r.get::<_,String>(1)?,"ref_to_message_id":r.get::<_,Option<String>>(2)?,"status":r.get::<_,String>(3)?,"attempts":r.get::<_,i64>(4)?,"error":error.map(|s|redact(&s)),"started_at":r.get::<_,Option<String>>(6)?,"available_at":r.get::<_,i64>(7)?}))
        }).optional()?;
        let mut query = db.prepare("SELECT id,kind,headers,at FROM callbacks WHERE id=? OR json_extract(headers, '$.\"x-holodeckb2b-reftomessageid\"')=? ORDER BY rowid")?;
        let rows = query.query_map(params![key, key], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let allowed = [
            "x-holodeckb2b-messageid",
            "x-holodeckb2b-reftomessageid",
            "x-holodeckb2b-conversationid",
            "x-holodeckb2b-pmodeid",
            "x-holodeckb2b-timestamp",
            "x-holodeckb2b-containment",
            "x-holodeckb2b-frompartyid",
            "x-holodeckb2b-topartyid",
            "x-holodeckb2b-service",
            "x-holodeckb2b-action",
            "x-holodeckb2b-errors",
            "content-type",
        ];
        let mut callbacks = Vec::new();
        let mut delivered_metadata = json!({});
        for row in rows {
            let (id, kind, raw, at) = row?;
            let headers: Value = serde_json::from_str(&raw)?;
            let mut metadata = serde_json::Map::new();
            for name in allowed {
                if let Some(value) = headers.get(name).and_then(Value::as_str) {
                    metadata.insert(name.into(), Value::String(redact(value)));
                }
            }
            let metadata = Value::Object(metadata);
            if id == key && kind == "deliver" {
                delivered_metadata = metadata.clone();
            }
            callbacks.push(json!({"id":id,"kind":kind,"at":at,"metadata":metadata}));
        }
        let pmode = delivered_metadata
            .get("x-holodeckb2b-pmodeid")
            .cloned()
            .unwrap_or_else(|| {
                json!(if message.direction == "out" {
                    &self.config.pmode_id
                } else {
                    &self.config.inbound_pmode_id
                })
            });
        let conversation = delivered_metadata
            .get("x-holodeckb2b-conversationid")
            .cloned()
            .unwrap_or_else(|| json!(message.process_id));
        let reference = outbox
            .as_ref()
            .and_then(|v| v.get("ref_to_message_id"))
            .cloned()
            .or_else(|| {
                delivered_metadata
                    .get("x-holodeckb2b-reftomessageid")
                    .cloned()
            })
            .unwrap_or(Value::Null);
        Ok(
            json!({"message":message,"business":business,"transport":{"message_id":key,"conversation_id":conversation,"ref_to_message_id":reference,"pmode_id":pmode,"direction":message.direction},"outbox":outbox,"callbacks":callbacks}),
        )
    }
}

/// Apply only the selected nomination profile to a submitted request. Keep the
/// full peer document on independent confirmations and in the immutable XML.
fn apply_confirmation(
    p: &mut Process,
    doc: &BusinessDocument,
    selected: Option<&edigas::NominationProfile>,
) {
    p.confirmation_document_id = Some(doc.id.clone());
    p.confirmation_version = Some(doc.version);
    p.confirmation_accepted = doc.accepted;
    p.confirmation_reasons = doc.reasons.clone();
    p.programme_profiles = doc.programme_profiles.clone();
    if doc.kind == "PROCON" {
        p.matching_statuses = doc.statuses.clone();
        p.stage = match doc.accepted {
            Some(true) => "Programme confirmed",
            Some(false) => "Programme rejected",
            None => "Programme review required",
        }
        .into();
        if doc.accepted == Some(true) {
            p.confirmed = doc.confirmed.clone();
        }
        return;
    }
    p.nomination_profiles = selected
        .map(|n| vec![n.clone()])
        .unwrap_or_else(|| doc.nomination_profiles.clone());
    if let Some(profile) = selected.or_else(|| doc.nomination_profiles.first()) {
        p.confirmed = profile.quantities.clone();
        p.matching_statuses = profile.statuses.clone();
        p.counterparty_quantities = profile.counterparty_quantities.clone();
        let codes: Vec<String> = profile.status_codes.iter().flatten().cloned().collect();
        p.stage = if profile.status_codes.iter().any(Vec::is_empty) {
            "Confirmation received".into()
        } else {
            result_stage("NOMINT", &codes)
        };
    }
}
fn reply_scope_failure(
    db: &Connection,
    p: &mut Process,
    m: &mut Message,
    detail: &str,
) -> Result<()> {
    m.process_id = Some(p.id.clone());
    m.transport_status = "Reply scope mismatch / review required".into();
    m.validation.valid = false;
    m.validation.errors.push(detail.into());
    m.validation.checks.push(edigas::ValidationCheck {
        layer: "Process context".into(),
        status: "fail".into(),
        detail: detail.into(),
    });
    event(
        p,
        "exception",
        "Unapplied reply: scope mismatch",
        detail,
        Some(&m.id),
    );
    process_save(db, p)
}
fn independent_confirmation(
    db: &Connection,
    doc: &BusinessDocument,
    validation: &ValidationReport,
    message: &str,
    origin: &str,
) -> Result<String> {
    let prior: Vec<Process> = query_json(
        db,
        "SELECT data FROM processes WHERE kind IN ('NOMRES','PROCON')",
    )?;
    let stream: Vec<_> = prior
        .iter()
        .filter(|p| p.kind == doc.kind && p.document_id == doc.id)
        .collect();
    let points: Vec<_> = doc
        .nomination_profiles
        .iter()
        .map(|n| n.point.as_str())
        .collect();
    if stream.iter().any(|p| {
        p.gas_day != doc.gas_day
            || p.nomination_profiles
                .iter()
                .any(|n| !points.contains(&n.point.as_str()))
    }) {
        return Err(Error::conflict(
            "Confirmation revision changes its gas day or point",
        ));
    }
    let newest = stream.iter().map(|p| p.version).max().unwrap_or(0);
    let current = doc.version > newest;
    let mut p = new_process(doc, validation, "Confirmation received", message);
    p.handoff_status = "Received".into();
    p.transport_status = "Delivered by AS4".into();
    p.ack_status = "Not applicable".into();
    p.confirmation_origin = Some(origin.into());
    apply_confirmation(&mut p, doc, None);
    if !current {
        p.stage = "Superseded confirmation".into();
    } else {
        for previous in stream {
            let mut previous = previous.clone();
            previous.stage = "Superseded confirmation".into();
            event(
                &mut previous,
                "evidence",
                "Later confirmation revision received",
                format!("{} / v{}", doc.id, doc.version),
                Some(message),
            );
            process_save(db, &previous)?;
        }
    }
    event(
        &mut p,
        "result",
        "Independent confirmation received",
        format!("{} / v{} / {}", doc.id, doc.version, origin),
        Some(message),
    );
    process_insert(db, &p, &doc.issuer)?;
    db.execute(
        "UPDATE processes SET result_version=? WHERE id=?",
        params![doc.version, p.id],
    )?;
    Ok(p.id)
}
fn receive_reply(
    db: &Connection,
    doc: &BusinessDocument,
    m: &mut Message,
    xml: &str,
) -> Result<Option<String>> {
    let reference_code = if doc.kind == "ACKNOW" {
        roxmltree::Document::parse(xml).ok().and_then(|tree| {
            tree.root_element()
                .children()
                .find(|n| {
                    n.is_element() && n.tag_name().name() == "receiving_Document.documentCode"
                })
                .and_then(|n| n.text())
                .map(str::to_owned)
        })
    } else {
        None
    };
    let expected_kind = if doc.kind == "PROCON" || reference_code.as_deref() == Some("ALH") {
        "PRODOC"
    } else {
        "NOMINT"
    };
    let mut key: Option<String> = if let Some(r) = &doc.reference {
        db.query_row(
            "SELECT id FROM processes WHERE document_id=? AND version=? AND issuer=? AND kind=?",
            params![r.id, r.version, edigas::BRP_EIC, expected_kind],
            |row| row.get(0),
        )
        .optional()?
    } else {
        None
    };
    if key.is_none() {
        if let Some(reference) = &doc.reference {
            key=db.query_row("SELECT id FROM processes WHERE document_id=? AND version=? AND issuer=? AND kind IN ('NOMINT','PRODOC')",params![reference.id,reference.version,edigas::BRP_EIC],|row|row.get(0)).optional()?;
        }
    }
    let aggregate = doc.kind == "NOMRES"
        && doc
            .nomination_profiles
            .iter()
            .any(|n| !matches!(n.point.as_str(), "TTF" | "21YNL----TTF---1"));
    if doc.kind == "NOMRES" && aggregate {
        return independent_confirmation(db, doc, &m.validation, &m.id, "aggregate").map(Some);
    }
    let Some(key) = key else {
        return if matches!(doc.kind.as_str(), "NOMRES" | "PROCON") {
            independent_confirmation(db, doc, &m.validation, &m.id, "unsolicited").map(Some)
        } else {
            Ok(None)
        };
    };
    let mut p = process_get(db, &key)?;
    if doc.kind == "ACKNOW" {
        let expected = if p.kind == "NOMINT" { "02G" } else { "ALH" };
        if reference_code.as_deref() != Some(expected) {
            reply_scope_failure(
                db,
                &mut p,
                m,
                "Acknowledgement references a different document family",
            )?;
            return Ok(None);
        }
        p.ack_status = if doc.accepted == Some(true) {
            "Accepted"
        } else {
            "Rejected"
        }
        .into();
        if p.confirmed.is_empty() {
            p.stage = p.ack_status.clone();
        }
        let detail = format!("{}: {}", p.ack_status, doc.reasons.join("; "));
        event(&mut p, "business", "ACKNOW received", detail, Some(&m.id));
        process_save(db, &p)?;
        return Ok(Some(key));
    }
    let selected = if doc.kind == "NOMRES" {
        doc.nomination_profiles.iter().find(|n| {
            n.counterparty == p.counterparty
                && n.direction == p.direction
                && matches!(n.point.as_str(), "TTF" | "21YNL----TTF---1")
        })
    } else {
        None
    };
    let scope_ok = doc.gas_day == p.gas_day
        && ((p.kind == "NOMINT" && doc.kind == "NOMRES" && selected.is_some())
            || (p.kind == "PRODOC" && doc.kind == "PROCON"));
    if !scope_ok {
        reply_scope_failure(db,&mut p,m,"Reply does not match the request's family, gas day, side or counterparty; confirmed quantities unchanged")?;
        return Ok(None);
    }
    let previous_version: u32 = db.query_row(
        "SELECT result_version FROM processes WHERE id=?",
        [&key],
        |row| row.get(0),
    )?;
    // Historical JSON predates explicit confirmation identity. Recover it from
    // actual received messages rather than comparing unrelated version numbers.
    let previous_id = p.confirmation_document_id.clone().or_else(|| {
        query_json::<Message>(
            db,
            "SELECT data FROM messages WHERE direction='in' ORDER BY rowid DESC",
        )
        .ok()?
        .into_iter()
        .find(|message| {
            message.process_id.as_deref() == Some(key.as_str()) && message.family == doc.kind
        })
        .map(|message| message.document_id)
    });
    if previous_id.as_ref().is_some_and(|id| id != &doc.id) {
        let independent = independent_confirmation(
            db,
            doc,
            &m.validation,
            &m.id,
            "different response identity / review",
        )?;
        event(
            &mut p,
            "evidence",
            "Different confirmation document retained",
            "Its version is independent; the current submitted process was not overwritten",
            Some(&m.id),
        );
        process_save(db, &p)?;
        return Ok(Some(independent));
    }
    if doc.version > previous_version {
        if selected
            .map(|n| n.quantities.len())
            .unwrap_or(doc.confirmed.len())
            != p.quantities.len()
        {
            reply_scope_failure(db, &mut p, m, "Incomplete result coverage")?;
            return Ok(None);
        }
        apply_confirmation(&mut p, doc, selected);
        if p.kind == "PRODOC" {
            p.programme_origin = Some("submitted".into());
        }
        db.execute(
            "UPDATE processes SET result_version=? WHERE id=?",
            params![doc.version, key],
        )?;
        let detail = p.stage.clone();
        event(
            &mut p,
            "result",
            &format!("{} received", doc.kind),
            detail,
            Some(&m.id),
        );
    } else {
        event(
            &mut p,
            "evidence",
            "Older result retained",
            format!(
                "{} v{} does not replace v{}",
                doc.id, doc.version, previous_version
            ),
            Some(&m.id),
        );
    }
    process_save(db, &p)?;
    if doc.kind == "NOMRES" && doc.nomination_profiles.len() > 1 {
        independent_confirmation(db, doc, &m.validation, &m.id, "consolidated").map(Some)
    } else {
        Ok(Some(key))
    }
}

fn query_json<T: serde::de::DeserializeOwned>(db: &Connection, sql: &str) -> Result<Vec<T>> {
    let mut q = db.prepare(sql)?;
    let values = q
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    values
        .into_iter()
        .map(|s| serde_json::from_str(&s).map_err(Error::from))
        .collect()
}
fn new_process(
    doc: &BusinessDocument,
    validation: &ValidationReport,
    stage: &str,
    source: &str,
) -> Process {
    Process {
        id: id(),
        document_id: doc.id.clone(),
        version: doc.version,
        kind: doc.kind.clone(),
        gas_day: doc.gas_day.clone(),
        counterparty: doc.counterparty.clone(),
        direction: doc.direction.clone(),
        quantities: doc.quantities.clone(),
        confirmed: vec![],
        counterparty_quantities: vec![],
        matching_statuses: vec![],
        stage: stage.into(),
        handoff_status: "Queued".into(),
        transport_status: "Awaiting receipt".into(),
        ack_status: "Awaiting".into(),
        validation: validation.clone(),
        events: vec![],
        created_at: now(),
        source_message_id: source.into(),
        balancing: doc.balancing.clone(),
        programme_origin: if doc.kind == "PRODOC" {
            Some("submitted".into())
        } else {
            None
        },
        nomination_profiles: if doc.kind == "NOMRES" {
            doc.nomination_profiles.clone()
        } else {
            vec![]
        },
        programme_profiles: if doc.kind == "PROCON" {
            doc.programme_profiles.clone()
        } else {
            vec![]
        },
        confirmation_accepted: doc.accepted,
        confirmation_reasons: doc.reasons.clone(),
        confirmation_document_id: if matches!(doc.kind.as_str(), "NOMRES" | "PROCON") {
            Some(doc.id.clone())
        } else {
            None
        },
        confirmation_version: if matches!(doc.kind.as_str(), "NOMRES" | "PROCON") {
            Some(doc.version)
        } else {
            None
        },
        confirmation_origin: None,
        programme_portfolio: if matches!(doc.kind.as_str(), "PRODOC" | "PROCON") {
            Some(edigas::BRP_ACCOUNT.into())
        } else {
            None
        },
    }
}
fn queue_xml(
    db: &Connection,
    p: &mut Process,
    xml: &str,
    validation: &ValidationReport,
    ref_to: Option<&str>,
) -> Result<String> {
    let doc = edigas::parse_document(xml)?;
    let transport = format!("{}@as4-lab", id());
    let m = Message {
        id: transport.clone(),
        family: doc.kind,
        code: doc.code,
        direction: "out".into(),
        document_id: doc.id,
        version: doc.version,
        process_id: Some(p.id.clone()),
        at: now(),
        transport_status: "Queued".into(),
        hash: hash(xml),
        xml: xml.into(),
        validation: validation.clone(),
    };
    message_insert(db, &m)?;
    db.execute(
        "INSERT INTO outbox(message_id,process_id,payload,ref_to,status) VALUES(?,?,?,?,'queued')",
        params![transport, p.id, xml, ref_to],
    )?;
    if p.source_message_id.is_empty() {
        p.source_message_id = transport.clone();
    }
    p.handoff_status = "Queued".into();
    p.transport_status = "Awaiting receipt".into();
    process_save(db, p)?;
    Ok(transport)
}
fn ensure_valid(r: &ValidationReport) -> Result<()> {
    if r.valid {
        Ok(())
    } else {
        Err(Error::bad(format!(
            "Generated response failed validation: {}",
            r.errors.join("; ")
        )))
    }
}
fn result_stage(kind: &str, statuses: &[String]) -> String {
    if kind == "PRODOC" {
        return "Programme confirmed".into();
    }
    if statuses.iter().any(|s| s == "13G") {
        "Unchanged settlement"
    } else if statuses.iter().any(|s| s == "14G") {
        "No counter-nomination"
    } else if statuses.iter().any(|s| s == "06G") {
        "Mismatch"
    } else if !statuses.is_empty() && statuses.iter().all(|s| s == "12G") {
        "Settled"
    } else {
        "Review required"
    }
    .into()
}

impl App {
    pub fn preview_balancing(&self, mut input: edigas::BalancingInput) -> Result<BalancingPreview> {
        self.role("tso")?;
        if input.document_id.is_none() {
            input.document_id = Some(format!("MAR{}", Uuid::new_v4().simple()));
        }
        let xml = edigas::generate_balancing(&input)?;
        let validation = edigas::validate_xml(&xml, &self.config.schema_dir);
        let preview = BalancingPreview {
            id: id(),
            input,
            hash: hash(&xml),
            xml,
            validation,
        };
        self.db.lock().unwrap().execute(
            "INSERT INTO balancing_previews(id,data) VALUES(?,?)",
            params![preview.id, serde_json::to_string(&preview)?],
        )?;
        Ok(preview)
    }
    pub fn send_balancing(&self, key: &str, review_hash: &str, command: &str) -> Result<String> {
        self.role("tso")?;
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let scope = format!("balancing:{key}");
        if let Some(value) = command_get(&tx, command, &scope, review_hash)? {
            return Ok(value["process_id"].as_str().unwrap_or_default().into());
        }
        let row: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT data,sent_process FROM balancing_previews WHERE id=?",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (raw, sent) = row.ok_or_else(|| Error::bad("Balancing preview not found"))?;
        if sent.is_some() {
            return Err(Error::conflict("This frozen allocation was already queued"));
        }
        let preview: BalancingPreview = serde_json::from_str(&raw)?;
        if preview.hash != review_hash || hash(&preview.xml) != review_hash {
            return Err(Error::conflict(
                "The allocation review hash does not match the frozen XML",
            ));
        }
        ensure_valid(&preview.validation)?;
        let doc = edigas::parse_document(&preview.xml)?;
        let previous: Vec<Process> =
            query_json(&tx, "SELECT data FROM processes WHERE kind='MARSIT'")?;
        for prior in previous.iter().filter(|p| p.document_id == doc.id) {
            if prior.version >= doc.version {
                return Err(Error::conflict(
                    "An equal or later allocation revision already exists",
                ));
            }
            if prior.gas_day != doc.gas_day
                || prior.balancing.as_ref().map(|b| b.document_code.as_str())
                    != Some(doc.code.as_str())
            {
                return Err(Error::conflict(
                    "Allocation revision changes its gas day or document code",
                ));
            }
        }
        let mut p = new_process(&doc, &preview.validation, "Allocation queued", "");
        p.ack_status = "Not applicable".into();
        process_insert(&tx, &p, &doc.issuer)?;
        let message = queue_xml(&tx, &mut p, &preview.xml, &preview.validation, None)?;
        event(
            &mut p,
            "allocation",
            "MARSIT allocation queued",
            format!("{} / v{} / {}", doc.id, doc.version, doc.code),
            Some(&message),
        );
        process_save(&tx, &p)?;
        tx.execute(
            "UPDATE balancing_previews SET sent_process=? WHERE id=?",
            params![p.id, key],
        )?;
        command_save(
            &tx,
            command,
            &scope,
            review_hash,
            json!({"process_id":p.id}),
        )?;
        tx.commit()?;
        Ok(p.id)
    }
    pub fn send_default_programme(&self, gas_day: &str, command: &str) -> Result<String> {
        self.role("tso")?;
        edigas::intervals(gas_day)?;
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        if let Some(value) = command_get(&tx, command, "default-programme", gas_day)? {
            return Ok(value["process_id"].as_str().unwrap_or_default().into());
        }
        let programmes: Vec<Process> = query_json(
            &tx,
            "SELECT data FROM processes WHERE kind IN ('PRODOC','PROCON')",
        )?;
        if programmes.iter().any(|p| p.gas_day == gas_day) {
            return Err(Error::conflict(
                "A programme already exists for this gas day; use its normal result flow",
            ));
        }
        let xml = edigas::generate_default_programme(gas_day, 1)?;
        let validation = edigas::validate_xml(&xml, &self.config.schema_dir);
        ensure_valid(&validation)?;
        let doc = edigas::parse_document(&xml)?;
        let mut p = new_process(&doc, &validation, "Default programme confirmed", "");
        p.programme_origin = Some("default".into());
        p.ack_status = "Not applicable".into();
        p.confirmed = doc.confirmed;
        process_insert(&tx, &p, &doc.issuer)?;
        let message = queue_xml(&tx, &mut p, &xml, &validation, None)?;
        event(
            &mut p,
            "programme",
            "Default PROCON queued",
            "Zero trade programme generated without a submitted PRODOC",
            Some(&message),
        );
        process_save(&tx, &p)?;
        command_save(
            &tx,
            command,
            "default-programme",
            gas_day,
            json!({"process_id":p.id}),
        )?;
        tx.commit()?;
        Ok(p.id)
    }
    pub fn receive_business(
        &self,
        transport_id: &str,
        headers: Value,
        xml: &str,
    ) -> Result<Option<String>> {
        if transport_id.is_empty() || transport_id.len() > 250 {
            return Err(Error::bad("Missing or invalid AS4 MessageId"));
        }
        if xml.len() > 1024 * 1024 {
            return Err(Error::bad("XML payload exceeds 1 MiB"));
        }
        let validation = edigas::validate_xml(xml, &self.config.schema_dir);
        let parsed = edigas::parse_document(xml);
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let digest = hash(xml);
        if callback_record(&tx, transport_id, "deliver", &headers, xml, &digest)? {
            tx.commit()?;
            return Ok(None);
        }
        let mut m = Message {
            id: transport_id.into(),
            family: parsed
                .as_ref()
                .map(|d| d.kind.clone())
                .unwrap_or("Unknown".into()),
            code: parsed.as_ref().map(|d| d.code.clone()).unwrap_or_default(),
            direction: "in".into(),
            document_id: parsed.as_ref().map(|d| d.id.clone()).unwrap_or_default(),
            version: parsed.as_ref().map(|d| d.version).unwrap_or(0),
            process_id: None,
            at: now(),
            transport_status: if validation.valid {
                "Received"
            } else {
                "Quarantined"
            }
            .into(),
            hash: digest.clone(),
            xml: xml.into(),
            validation: validation.clone(),
        };
        let doc = match parsed {
            Ok(d) => d,
            Err(_) => {
                message_insert(&tx, &m)?;
                tx.commit()?;
                return Ok(None);
            }
        };
        let existing:Option<String>=tx.query_row("SELECT hash FROM business_facts WHERE issuer=? AND document_id=? AND version=? AND code=?",params![doc.issuer,doc.id,doc.version,doc.code],|r|r.get(0)).optional()?;
        if let Some(old) = existing {
            m.transport_status = if old == digest {
                "Duplicate business document"
            } else {
                "Conflicting business identity"
            }
            .into();
            message_insert(&tx, &m)?;
            tx.commit()?;
            return Ok(None);
        }
        if !validation.valid {
            message_insert(&tx, &m)?;
            tx.commit()?;
            return Ok(None);
        }
        let correct_party = if self.config.role == "tso" {
            doc.issuer == edigas::BRP_EIC && doc.recipient == edigas::TSO_EIC
        } else {
            doc.issuer == edigas::TSO_EIC && doc.recipient == edigas::BRP_EIC
        };
        if !correct_party {
            m.transport_status = "Unexpected party identity".into();
            message_insert(&tx, &m)?;
            tx.commit()?;
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO business_facts(issuer,document_id,version,code,hash) VALUES(?,?,?,?,?)",
            params![doc.issuer, doc.id, doc.version, doc.code, digest],
        )?;
        let process_id = if self.config.role == "tso"
            && ["NOMINT", "PRODOC"].contains(&doc.kind.as_str())
        {
            let previous:Option<String>=tx.query_row("SELECT data FROM processes WHERE document_id=? AND issuer=? ORDER BY version DESC LIMIT 1",params![doc.id,doc.issuer],|r|r.get(0)).optional()?;
            if let Some(raw) = previous {
                let prior: Process = serde_json::from_str(&raw)?;
                if prior.kind != doc.kind
                    || prior.gas_day != doc.gas_day
                    || prior.direction != doc.direction
                    || prior.counterparty != doc.counterparty
                {
                    m.transport_status = "Revision scope mismatch / quarantined".into();
                    message_insert(&tx, &m)?;
                    tx.commit()?;
                    return Ok(None);
                }
            }
            let mut p = new_process(&doc, &validation, "Received", transport_id);
            p.handoff_status = "Received".into();
            p.transport_status = "Delivered by AS4".into();
            event(
                &mut p,
                "received",
                "Business document received",
                format!("{} {} / v{}", doc.kind, doc.id, doc.version),
                Some(transport_id),
            );
            process_insert(&tx, &p, &doc.issuer)?;
            Some(p.id)
        } else if self.config.role == "brp" && doc.kind == "MARSIT" {
            let prior: Vec<Process> =
                query_json(&tx, "SELECT data FROM processes WHERE kind='MARSIT'")?;
            let stream: Vec<&Process> = prior.iter().filter(|p| p.document_id == doc.id).collect();
            if stream.iter().any(|p| {
                p.gas_day != doc.gas_day
                    || p.balancing.as_ref().map(|b| b.document_code.as_str())
                        != Some(doc.code.as_str())
            }) {
                m.transport_status = "Allocation revision scope mismatch / quarantined".into();
                message_insert(&tx, &m)?;
                tx.commit()?;
                return Ok(None);
            }
            let newest = stream.iter().map(|p| p.version).max().unwrap_or(0);
            let mut p = new_process(
                &doc,
                &validation,
                if doc.version > newest {
                    "Allocation received"
                } else {
                    "Superseded allocation"
                },
                transport_id,
            );
            p.handoff_status = "Received".into();
            p.transport_status = "Delivered by AS4".into();
            p.ack_status = "Not applicable".into();
            if doc.version > newest {
                for old in stream {
                    let mut old = old.clone();
                    old.stage = "Superseded allocation".into();
                    event(
                        &mut old,
                        "evidence",
                        "Later allocation revision received",
                        format!("Revision {} replaces this allocation", doc.version),
                        Some(transport_id),
                    );
                    process_save(&tx, &old)?;
                }
            }
            event(
                &mut p,
                "allocation",
                "MARSIT received",
                format!("{} / v{}; each transaction retained", doc.code, doc.version),
                Some(transport_id),
            );
            process_insert(&tx, &p, &doc.issuer)?;
            Some(p.id)
        } else if self.config.role == "brp"
            && doc.kind == "PROCON"
            && doc.reference.as_ref().is_some_and(|r| r.id == "DEFAULT")
        {
            // DEFAULT does not identify a submitted PRODOC. Portfolio/day is a
            // logical association only; revisions are scoped to the actual
            // issuer and confirmation document ID, never another submission.
            let prior: Vec<Process> = {
                let mut query = tx.prepare(
                    "SELECT data FROM processes WHERE kind='PROCON' AND issuer=? AND document_id=?",
                )?;
                let values = query
                    .query_map(params![doc.issuer, doc.id], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                values
                    .into_iter()
                    .map(|raw| serde_json::from_str(&raw))
                    .collect::<std::result::Result<_, _>>()?
            };
            if prior.iter().any(|p| {
                p.gas_day != doc.gas_day || p.programme_origin.as_deref() != Some("default")
            }) {
                m.transport_status =
                    "Default programme revision scope mismatch / quarantined".into();
                message_insert(&tx, &m)?;
                tx.commit()?;
                return Ok(None);
            }
            let newest = prior.iter().map(|p| p.version).max().unwrap_or(0);
            let is_latest = doc.version > newest;
            let mut p = new_process(
                &doc,
                &validation,
                if is_latest {
                    "Default programme confirmed"
                } else {
                    "Superseded default programme"
                },
                transport_id,
            );
            p.ack_status = "Not applicable".into();
            p.handoff_status = "Received".into();
            p.transport_status = "Delivered by AS4".into();
            p.programme_origin = Some("default".into());
            apply_confirmation(&mut p, &doc, None);
            if is_latest && doc.accepted == Some(true) {
                p.stage = "Default programme confirmed".into();
            }
            if !is_latest {
                p.stage = "Superseded default programme".into();
            }
            if is_latest {
                for mut old in prior {
                    old.stage = "Superseded default programme".into();
                    event(
                        &mut old,
                        "evidence",
                        "Later DEFAULT confirmation revision received",
                        format!(
                            "Version {} replaces this version of the same response document",
                            doc.version
                        ),
                        Some(transport_id),
                    );
                    process_save(&tx, &old)?;
                }
            }
            event(
                &mut p,
                "programme",
                if is_latest {
                    "DEFAULT PROCON received"
                } else {
                    "Older DEFAULT PROCON retained"
                },
                format!(
                    "Synthetic portfolio {} / gas day {}; independent confirmation version {}",
                    edigas::BRP_ACCOUNT,
                    doc.gas_day,
                    doc.version
                ),
                Some(transport_id),
            );
            process_insert(&tx, &p, &doc.issuer)?;
            tx.execute(
                "UPDATE processes SET result_version=? WHERE id=?",
                params![doc.version, p.id],
            )?;
            Some(p.id)
        } else if self.config.role == "brp"
            && ["ACKNOW", "NOMRES", "PROCON"].contains(&doc.kind.as_str())
        {
            receive_reply(&tx, &doc, &mut m, xml)?
        } else {
            None
        };
        if process_id.is_none() && m.process_id.is_none() {
            m.transport_status = "Unmatched / review required".into();
        }
        if process_id.is_some() {
            m.process_id = process_id.clone();
        }
        message_insert(&tx, &m)?;
        tx.commit()?;
        Ok(process_id)
    }
    pub fn receive_signal(&self, kind: &str, headers: Value) -> Result<()> {
        let transport = headers
            .get("x-holodeckb2b-messageid")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::bad("Missing signal MessageId"))?;
        let reference = headers
            .get("x-holodeckb2b-reftomessageid")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let body = headers.to_string();
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        if callback_record(&tx, transport, kind, &headers, "", &hash(&body))? {
            tx.commit()?;
            return Ok(());
        }
        let found: Option<String> = tx
            .query_row(
                "SELECT process_id FROM outbox WHERE message_id=?",
                [reference],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(key) = found {
            let mut p = process_get(&tx, &key)?;
            let latest: String = tx.query_row(
                "SELECT message_id FROM outbox WHERE process_id=? ORDER BY id DESC LIMIT 1",
                [&key],
                |r| r.get(0),
            )?;
            let signal_status = if kind == "receipt" {
                "Receipt received"
            } else {
                "AS4 error"
            };
            let mut source = message_get(&tx, reference)?;
            source.transport_status = signal_status.into();
            tx.execute(
                "UPDATE messages SET data=? WHERE id=?",
                params![serde_json::to_string(&source)?, reference],
            )?;
            if latest == reference {
                p.transport_status = signal_status.into();
            }
            if kind == "receipt" && latest == reference && p.handoff_status == "Delivery unknown" {
                p.handoff_status = "Accepted (receipt evidence)".into();
                if p.stage == "Delivery unknown" {
                    p.stage = "Awaiting ACKNOW".into()
                }
            }
            let detail = headers
                .get("x-holodeckb2b-errors")
                .and_then(Value::as_str)
                .unwrap_or(reference);
            event(
                &mut p,
                "transport",
                if kind == "receipt" {
                    "AS4 receipt received"
                } else {
                    "AS4 error reported"
                },
                detail,
                Some(reference),
            );
            process_save(&tx, &p)?;
            if kind == "receipt" {
                tx.execute("UPDATE outbox SET status='accepted',error=NULL WHERE message_id=? AND status IN ('unknown','dispatching','queued')",[reference])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn callback_record(
    db: &Connection,
    key: &str,
    kind: &str,
    headers: &Value,
    body: &str,
    digest: &str,
) -> Result<bool> {
    let found: Option<String> = db
        .query_row("SELECT hash FROM callbacks WHERE id=?", [key], |r| r.get(0))
        .optional()?;
    if let Some(old) = found {
        return if old == digest {
            Ok(true)
        } else {
            Err(Error::conflict(
                "Same AS4 message identity arrived with conflicting content",
            ))
        };
    }
    db.execute(
        "INSERT INTO callbacks(id,hash,kind,headers,body,at) VALUES(?,?,?,?,?,?)",
        params![key, digest, kind, headers.to_string(), body, now()],
    )?;
    Ok(false)
}
