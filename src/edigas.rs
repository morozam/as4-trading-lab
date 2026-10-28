//! Deliberately bounded, offline Edigas 6.1 TTF laboratory profile.
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Europe::Amsterdam;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use uuid::Uuid;

pub const BRP_EIC: &str = "99X-LAB-BRP----0";
pub const TSO_EIC: &str = "99X-LAB-TSO----0";
pub const BRP_ACCOUNT: &str = "GSLABBRP";
pub const COUNTERPARTY_ACCOUNT: &str = "GSLABCOUNTER";
const MAX_XML: usize = 1_000_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hour {
    pub start: String,
    pub end: String,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DraftInput {
    pub kind: String,
    pub gas_day: String,
    pub counterparty: String,
    pub direction: String,
    pub quantities: Vec<i64>,
    pub document_id: Option<String>,
    pub version: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentRef {
    pub id: String,
    pub version: u32,
}
/// GTS MARSIT uses one period per transaction; prices remain decimal strings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalancingPeriod {
    pub start: String,
    pub end: String,
    pub direction: String,
    pub quantity: i64,
    pub price: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalancingTransaction {
    pub identification: String,
    pub periods: Vec<BalancingPeriod>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalancingInput {
    pub gas_day: String,
    pub document_code: String,
    pub document_id: Option<String>,
    pub version: u32,
    pub transactions: Vec<BalancingTransaction>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalancingDetail {
    pub document_code: String,
    pub portfolio: String,
    pub market_area: String,
    pub unit: String,
    pub currency: String,
    pub price_unit: String,
    pub transactions: Vec<BalancingTransaction>,
}
/// One counterportfolio at one network point. Hourly sides are kept separately
/// because confirmations may change direction within a gas day.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NominationProfile {
    pub internal_account: String,
    pub point: String,
    pub point_scheme: String,
    pub nomination_code: Option<String>,
    pub counterparty: String,
    pub direction: String,
    pub directions: Vec<String>,
    pub quantities: Vec<i64>,
    /// First code per hour for existing consumers; empty means no Status supplied.
    pub statuses: Vec<String>,
    /// Every supplied Status code, without interpreting unknown codes as settled.
    pub status_codes: Vec<Vec<String>>,
    pub counterparty_quantities: Vec<i64>,
    #[serde(default)]
    pub counterparty_directions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgrammeProfile {
    pub account: String,
    pub account_code: String,
    pub quantities: Vec<i64>,
    pub reason_codes: Vec<Vec<String>>,
    pub reason_texts: Vec<Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusinessDocument {
    #[serde(default)]
    pub creation_date_time: String,
    #[serde(default)]
    pub nomination_profiles: Vec<NominationProfile>,
    #[serde(default)]
    pub programme_profiles: Vec<ProgrammeProfile>,
    pub id: String,
    pub version: u32,
    pub code: String,
    pub kind: String,
    pub gas_day: String,
    pub issuer: String,
    pub recipient: String,
    pub reference: Option<DocumentRef>,
    pub direction: String,
    pub quantities: Vec<i64>,
    pub counterparty: String,
    pub statuses: Vec<String>,
    pub confirmed: Vec<i64>,
    pub counterparty_quantities: Vec<i64>,
    pub accepted: Option<bool>,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub balancing: Option<BalancingDetail>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchingResult {
    pub quantities: Vec<i64>,
    pub statuses: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub layer: String,
    pub status: String,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
    pub errors: Vec<String>,
}

pub fn intervals(day: &str) -> Result<Vec<Hour>, String> {
    let day =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| "Expected gas day YYYY-MM-DD")?;
    let next = day.succ_opt().ok_or("Gas day out of range")?;
    let start = Amsterdam
        .from_local_datetime(&day.and_hms_opt(6, 0, 0).unwrap())
        .single()
        .ok_or("Invalid gas day start")?
        .with_timezone(&Utc);
    let end = Amsterdam
        .from_local_datetime(&next.and_hms_opt(6, 0, 0).unwrap())
        .single()
        .ok_or("Invalid gas day end")?
        .with_timezone(&Utc);
    let mut hours = Vec::new();
    let mut time = start;
    while time < end {
        let next = time + Duration::hours(1);
        hours.push(Hour {
            start: iso(time),
            end: iso(next),
            label: time
                .with_timezone(&Amsterdam)
                .format("%H:%M %Z (UTC%:z)")
                .to_string(),
        });
        time = next;
    }
    Ok(hours)
}
fn iso(time: DateTime<Utc>) -> String {
    time.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
fn xml_time(s: &str) -> String {
    s.replace(":00Z", "Z")
}
fn span(h: &Hour) -> String {
    format!("{}/{}", xml_time(&h.start), xml_time(&h.end))
}
fn validity(day: &str) -> Result<String, String> {
    let h = intervals(day)?;
    Ok(format!(
        "{}/{}",
        xml_time(&h[0].start),
        xml_time(&h.last().unwrap().end)
    ))
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn el(name: &str, s: &str) -> String {
    format!("<{name}>{}</{name}>\n", escape(s))
}
fn coded(name: &str, s: &str, scheme: &str) -> String {
    format!("<{name} codingScheme=\"{scheme}\">{}</{name}>\n", escape(s))
}
fn tag<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Result<&'a str, String> {
    let mut n = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == name);
    let first = n.next().ok_or_else(|| format!("Missing {name}"))?;
    if n.next().is_some() {
        return Err(format!("Duplicate {name}"));
    }
    first.text().ok_or_else(|| format!("Empty {name}"))
}
fn children<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Vec<Node<'a, 'i>> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name() == name)
        .collect()
}
fn one<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Result<Node<'a, 'i>, String> {
    let n = children(node, name);
    if n.len() != 1 {
        return Err(format!("Local profile requires exactly one {name}"));
    }
    Ok(n[0])
}
fn ver(s: &str) -> Result<u32, String> {
    let v = s.parse::<u32>().map_err(|_| "Invalid version")?;
    if !(1..=999).contains(&v) {
        return Err("Version must be 1..999".into());
    }
    Ok(v)
}
fn ident(s: &str) -> Result<(), String> {
    if s.is_empty()
        || s.len() > 35
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("Document ID must be 1..35 ASCII letters, digits, -, _ or .".into());
    }
    Ok(())
}

struct Family {
    kind: &'static str,
    root: &'static str,
    area: &'static str,
    name: &'static str,
    code: &'static str,
}
const FAMILIES: [Family; 6] = [
    Family {
        kind: "MARSIT",
        root: "MarketSituation_Document",
        area: "BalancingAndSettlement",
        name: "MarketSituationDocument",
        code: "ANZ",
    },
    Family {
        kind: "NOMINT",
        root: "Nomination_Document",
        area: "BrpNominationAndMatching",
        name: "NominationDocument",
        code: "02G",
    },
    Family {
        kind: "NOMRES",
        root: "NominationResponse_Document",
        area: "BrpNominationAndMatching",
        name: "NominationResponseDocument",
        code: "08G",
    },
    Family {
        kind: "ACKNOW",
        root: "Acknowledgement_Document",
        area: "General",
        name: "AcknowledgementDocument",
        code: "294",
    },
    Family {
        kind: "PRODOC",
        root: "LoadForecast_Document",
        area: "General",
        name: "LoadForecastDocument",
        code: "ALH",
    },
    Family {
        kind: "PROCON",
        root: "LoadForecastConfirmation_Document",
        area: "General",
        name: "LoadForecastConfirmationDocument",
        code: "ALK",
    },
];
fn family(kind: &str) -> Result<&'static Family, String> {
    FAMILIES
        .iter()
        .find(|f| f.kind == kind)
        .ok_or_else(|| "Unsupported document family".into())
}
fn namespace(f: &Family) -> String {
    format!("urn:easee-gas.eu:edigas:{}:{}:6:1", f.area, f.name)
}
fn header(
    f: &Family,
    id: &str,
    version: u32,
    day: Option<&str>,
    response: bool,
) -> Result<String, String> {
    ident(id)?;
    ver(&version.to_string())?;
    let mut x = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{} xmlns=\"{}\" schemaVersion=\"1\">\n",
        f.root,
        namespace(f)
    );
    x += &el("identification", id);
    x += &el("version", &version.to_string());
    x += &el("documentCode", f.code);
    x += &el("creationDateTime", &iso(Utc::now()));
    if let Some(day) = day {
        x += &el("validityPeriod", &validity(day)?);
    }
    let programme = f.kind.starts_with("PRO");
    let tso_role = if programme || f.kind == "ACKNOW" {
        "ZSO"
    } else {
        "ZUK"
    };
    if programme {
        x += &el("contract_Reference.identification", BRP_ACCOUNT);
        x += &el(
            "contract_Reference.referenceCode",
            if response { "CT" } else { "ZSC" },
        );
    }
    x += &coded(
        "issuer_MarketParticipant.identification",
        if response { TSO_EIC } else { BRP_EIC },
        "305",
    );
    x += &el(
        if programme {
            "issuer_MarketParticipant.issuer_MarketRole.roleCode"
        } else {
            "issuer_MarketParticipant.marketRole.roleCode"
        },
        if response { tso_role } else { "ZSH" },
    );
    x += &coded(
        "recipient_MarketParticipant.identification",
        if response { BRP_EIC } else { TSO_EIC },
        "305",
    );
    x += &el(
        if programme {
            "recipient_MarketParticipant.recipient_MarketRole.roleCode"
        } else {
            "recipient_MarketParticipant.marketRole.roleCode"
        },
        if response { "ZSH" } else { tso_role },
    );
    Ok(x)
}
fn direction_code(s: &str) -> Result<&'static str, String> {
    match s {
        "buy" => Ok("Z02"),
        "sell" => Ok("Z03"),
        _ => Err("Direction must be buy or sell".into()),
    }
}
fn periods(
    hours: &[Hour],
    q: &[i64],
    dir: &str,
    statuses: Option<&[String]>,
    programme_result: bool,
) -> String {
    let mut x = String::new();
    for (i, h) in hours.iter().enumerate() {
        x += "<Period>\n";
        x += &el("timeInterval", &span(h));
        x += &el("direction.gasDirectionCode", dir);
        x += &el("quantity.amount", &q[i].to_string());
        if let Some(st) = statuses {
            x += "<Status>";
            x += &el("statusCode", &st[i]);
            x += "</Status>\n";
        }
        if programme_result {
            x += "<Reason><reasonCode>01G</reasonCode></Reason>\n";
        }
        x += "</Period>\n";
    }
    x
}
fn nomination_open(counter: &str) -> String {
    format!("<Internal_Account>\n{}<ConnectionPoint>\n{}<measureUnit.unitOfMeasureCode>KW1</measureUnit.unitOfMeasureCode>\n<NominationType><nominationCode>A02</nominationCode><External_Account>\n{}",coded("internalAccount",BRP_ACCOUNT,"ZSO"),coded("identification","TTF","ZSO"),coded("externalAccount",counter,"ZSO"))
}
const NOM_CLOSE: &str =
    "</External_Account></NominationType></ConnectionPoint></Internal_Account>\n";
fn programme_open() -> String {
    format!(
        "<ConnectionPoint>\n{}<measureUnit.unitOfMeasureCode>KW1</measureUnit.unitOfMeasureCode>\n",
        coded("identification", "VPPV", "ZSO")
    )
}
fn account(name: &str, code: &str, hours: &[Hour], q: &[i64], dir: &str, result: bool) -> String {
    format!(
        "<Account>\n{}{}{} </Account>\n",
        coded("identification", name, "ZSO"),
        el("accountCode", code),
        periods(hours, q, dir, None, result)
    )
}

pub fn generate_submission(input: &DraftInput) -> Result<String, String> {
    if !matches!(input.kind.as_str(), "NOMINT" | "PRODOC") {
        return Err("Only NOMINT or PRODOC can be submitted".into());
    }
    let f = family(&input.kind)?;
    let h = intervals(&input.gas_day)?;
    let dir = direction_code(&input.direction)?;
    if input.quantities.len() != h.len() || input.quantities.iter().any(|q| *q < 0) {
        return Err("A complete nonnegative hourly gas-day profile is required".into());
    }
    if input.counterparty != COUNTERPARTY_ACCOUNT {
        return Err("This lab supports only the synthetic counterparty".into());
    }
    if input.kind == "PRODOC" && input.quantities.iter().any(|q| *q != 0) {
        return Err("Trade programme quantities must all be zero".into());
    }
    let id = input
        .document_id
        .as_deref()
        .ok_or("Backend document_id is required")?;
    let mut x = header(f, id, input.version, Some(&input.gas_day), false)?;
    if input.kind == "NOMINT" {
        x += &nomination_open(&input.counterparty);
        x += &periods(&h, &input.quantities, dir, None, false);
        x += NOM_CLOSE;
    } else {
        x += &programme_open();
        x += &account("GSTPTRADE", "ZOC", &h, &input.quantities, dir, false);
        x += "</ConnectionPoint>\n";
    }
    x += &format!("</{}>\n", f.root);
    Ok(x)
}

pub fn match_nomination(
    requested: &[i64],
    counterparty: Option<&[i64]>,
    previously_settled: Option<&[Option<i64>]>,
) -> Result<MatchingResult, String> {
    if requested.is_empty()
        || requested.iter().any(|v| *v < 0)
        || counterparty.is_some_and(|v| v.len() != requested.len() || v.iter().any(|x| *x < 0))
        || previously_settled
            .is_some_and(|v| v.len() != requested.len() || v.iter().flatten().any(|x| *x < 0))
    {
        return Err("Matching requires equally sized nonnegative profiles".into());
    }
    let mut r = MatchingResult {
        quantities: Vec::new(),
        statuses: Vec::new(),
    };
    for (i, req) in requested.iter().enumerate() {
        let cp = counterparty.map(|c| c[i]);
        let prior = previously_settled.and_then(|s| s[i]);
        let (q, st) = if cp == Some(*req) {
            (*req, "12G")
        } else if let Some(p) = prior {
            (p, "13G")
        } else if let Some(c) = cp {
            ((*req).min(c), "06G")
        } else {
            (0, "14G")
        };
        r.quantities.push(q);
        r.statuses.push(st.into());
    }
    Ok(r)
}

pub fn generate_ack(
    request: &BusinessDocument,
    accepted: bool,
    reason: &str,
) -> Result<String, String> {
    if !matches!(request.kind.as_str(), "NOMINT" | "PRODOC") {
        return Err("ACKNOW must reference a submission".into());
    }
    if reason.chars().count() > 512 || (!accepted && reason.trim().is_empty()) {
        return Err("Rejection requires a reason of at most 512 characters".into());
    }
    if reason.chars().any(|c| !matches!(c, '\u{9}'|'\u{a}'|'\u{d}'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}')) {
        return Err("Reason contains a character forbidden by XML 1.0".into());
    }
    let f = family("ACKNOW")?;
    let id = format!("ACK{}", Uuid::new_v4().simple());
    let mut x = header(f, &id, 1, None, true)?;
    if request.kind == "NOMINT" {
        x = x.replace("<issuer_MarketParticipant.marketRole.roleCode>ZSO</issuer_MarketParticipant.marketRole.roleCode>", "<issuer_MarketParticipant.marketRole.roleCode>ZUK</issuer_MarketParticipant.marketRole.roleCode>");
    }
    x += &el("receiving_Document.identification", &request.id);
    x += &el("receiving_Document.version", &request.version.to_string());
    x += &el("receiving_Document.documentCode", &request.code);
    // This must be the acknowledged document's creation time, never ACK time.
    parse_time(&request.creation_date_time)
        .map_err(|_| "Original document creationDateTime is required for ACKNOW")?;
    x += &el(
        "receiving_Document.creationDateTime",
        &request.creation_date_time,
    );
    x += "<Reason>\n";
    x += &el("reasonCode", if accepted { "01G" } else { "91G" });
    if !reason.is_empty() {
        x += &el("text", reason);
    }
    x += "</Reason>\n</Acknowledgement_Document>\n";
    Ok(x)
}

/// A response series retains its identity across request revisions and reruns.
/// The caller assigns the next version from its persisted outbound history.
pub fn response_document_id(request: &BusinessDocument) -> String {
    let kind = if request.kind == "NOMINT" {
        "NOMRES"
    } else {
        "PROCON"
    };
    let mut hash = Sha256::new();
    for part in [kind, &request.issuer, &request.id, &request.gas_day] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("RES{}", &hex::encode(hash.finalize())[..32])
}

pub fn generate_result(
    request: &BusinessDocument,
    result: &MatchingResult,
    sequence: u32,
) -> Result<String, String> {
    let kind = match request.kind.as_str() {
        "NOMINT" => "NOMRES",
        "PRODOC" => "PROCON",
        _ => return Err("Result must reference a submission".into()),
    };
    let h = intervals(&request.gas_day)?;
    let dir = direction_code(&request.direction)?;
    if result.quantities.len() != h.len()
        || result.statuses.len() != h.len()
        || result.quantities.iter().any(|q| *q < 0)
        || result
            .statuses
            .iter()
            .any(|s| !matches!(s.as_str(), "12G" | "13G" | "14G" | "06G"))
    {
        return Err("Invalid matching result profile".into());
    }
    let f = family(kind)?;
    let id = response_document_id(request);
    let mut x = header(f, &id, sequence, Some(&request.gas_day), true)?;
    if kind == "NOMRES" {
        x += &el("nomination_Document.identification", &request.id);
        x += &el("nomination_Document.version", &request.version.to_string());
        x += &el("nomination_Document.documentCode", &request.code);
        x += &nomination_open(&request.counterparty);
        x += "<InformationOrigin_TimeSeries><businessCode>16G</businessCode>\n";
        x += &periods(&h, &result.quantities, dir, Some(&result.statuses), false);
        x += "</InformationOrigin_TimeSeries>\n";
        if !request.counterparty_quantities.is_empty() {
            if request.counterparty_quantities.len() != h.len()
                || request.counterparty_quantities.iter().any(|q| *q < 0)
            {
                return Err("Invalid counterparty profile".into());
            }
            x += "<InformationOrigin_TimeSeries><businessCode>18G</businessCode>\n";
            x += &periods(
                &h,
                &request.counterparty_quantities,
                if dir == "Z02" { "Z03" } else { "Z02" },
                None,
                false,
            );
            x += "</InformationOrigin_TimeSeries>\n";
        }
        x += NOM_CLOSE;
    } else {
        if result.quantities.iter().any(|q| *q != 0) {
            return Err("Trade programme confirmation must be zero".into());
        }
        x += &el("proDoc_Document.identification", &request.id);
        x += &el("proDoc_Document.version", &request.version.to_string());
        x += &programme_open();
        for (name, code, d) in [
            ("GSTPTRADE", "ZUD", dir),
            ("GSTPD", "ZTX", dir),
            ("GSTPVPPVEN", "ZUD", "Z03"),
            ("GSTPVPPVEX", "ZUD", "Z03"),
        ] {
            x += &account(name, code, &h, &result.quantities, d, true);
        }
        x += "</ConnectionPoint>\n";
    }
    x += &format!("</{}>\n", f.root);
    Ok(x)
}

fn safe_parse(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > MAX_XML {
        return Err("XML exceeds 1 MB limit".into());
    }
    let upper = xml.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err("DOCTYPE and entity declarations are forbidden".into());
    }
    Document::parse(xml).map_err(|e| format!("Malformed XML: {e}"))
}
fn detect(root: Node<'_, '_>) -> Result<&'static Family, String> {
    let f = FAMILIES
        .iter()
        .find(|f| {
            root.tag_name().name() == f.root
                && root.tag_name().namespace() == Some(namespace(f).as_str())
        })
        .ok_or("Unsupported root QName (Edigas 6.1 required)")?;
    if tag(root, "documentCode")? != f.code
        && !(f.kind == "MARSIT" && tag(root, "documentCode")? == "AVI")
    {
        return Err(format!("{} requires documentCode {}", f.kind, f.code));
    }
    if root
        .descendants()
        .filter(|n| n.is_element())
        .any(|n| n.tag_name().namespace() != root.tag_name().namespace())
    {
        return Err("Every business element must use the document namespace".into());
    }
    Ok(f)
}
fn parse_time(s: &str) -> Result<DateTime<Utc>, String> {
    // Edigas interval lexical form has minute precision; chrono RFC3339 expects seconds.
    let s = if s.len() == 17 && s.ends_with('Z') {
        format!("{}:00Z", &s[..16])
    } else {
        s.to_string()
    };
    DateTime::parse_from_rfc3339(&s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| "Invalid UTC timestamp".into())
}
fn parse_span(s: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    let (a, b) = s.split_once('/').ok_or("Invalid interval")?;
    if !a.ends_with('Z') || !b.ends_with('Z') {
        return Err("Intervals must use UTC Z".into());
    }
    let (a, b) = (parse_time(a)?, parse_time(b)?);
    if b <= a {
        return Err("Interval end must follow start".into());
    }
    Ok((a, b))
}
fn gas_day(s: &str) -> Result<String, String> {
    let (start, end) = parse_span(s)?;
    let d = start.with_timezone(&Amsterdam).date_naive().to_string();
    let h = intervals(&d)?;
    if start != parse_time(&h[0].start)? || end != parse_time(&h.last().unwrap().end)? {
        return Err("Validity must be one Europe/Amsterdam 06:00 gas day".into());
    }
    Ok(d)
}
struct ParsedProfile {
    quantities: Vec<i64>,
    statuses: Vec<String>,
    status_codes: Vec<Vec<String>>,
    direction: String,
    directions: Vec<String>,
    reason_codes: Vec<Vec<String>>,
    reason_texts: Vec<Vec<String>>,
}
fn profile(node: Node<'_, '_>, hours: &[Hour]) -> Result<ParsedProfile, String> {
    let mut quantities = vec![None; hours.len()];
    let mut directions = vec![String::new(); hours.len()];
    let mut status_codes = vec![vec![]; hours.len()];
    let mut reason_codes = vec![vec![]; hours.len()];
    let mut reason_texts = vec![vec![]; hours.len()];
    for p in children(node, "Period") {
        let (a, b) = parse_span(tag(p, "timeInterval")?)?;
        let value = tag(p, "quantity.amount")?
            .parse::<i64>()
            .map_err(|_| "Local profile requires natural integer kWh/h quantities")?;
        if value < 0 {
            return Err("Quantity must be nonnegative".into());
        }
        let direction = match tag(p, "direction.gasDirectionCode")? {
            "Z02" => "buy",
            "Z03" => "sell",
            _ => return Err("Unsupported gas direction".into()),
        };
        // Status is optional and repeatable in the supplied NOMRES XSD.
        // Preserve the codelist values; only matching logic may create 12G.
        let statuses = children(p, "Status")
            .iter()
            .map(|n| tag(*n, "statusCode").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        let reasons = children(p, "Reason");
        let codes = reasons
            .iter()
            .map(|n| tag(*n, "reasonCode").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        let texts = reasons
            .iter()
            .map(|n| tag(*n, "text").unwrap_or("").to_owned())
            .collect::<Vec<_>>();
        let mut count = 0;
        for (i, h) in hours.iter().enumerate() {
            if parse_time(&h.start)? >= a && parse_time(&h.end)? <= b {
                if quantities[i].is_some() {
                    return Err("Overlapping periods".into());
                }
                quantities[i] = Some(value);
                directions[i] = direction.into();
                status_codes[i] = statuses.clone();
                reason_codes[i] = codes.clone();
                reason_texts[i] = texts.clone();
                count += 1;
            }
        }
        if count == 0
            || a < parse_time(&hours[0].start)?
            || b > parse_time(&hours.last().unwrap().end)?
            || (b - a).num_seconds() != count * 3600
        {
            return Err("Periods must cover whole gas-day hours without gaps".into());
        }
    }
    let quantities = quantities
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("Incomplete hourly profile")?;
    let direction = if directions.iter().all(|d| d == &directions[0]) {
        directions[0].clone()
    } else {
        "mixed".into()
    };
    let statuses = status_codes
        .iter()
        .map(|codes| codes.first().cloned().unwrap_or_default())
        .collect();
    Ok(ParsedProfile {
        quantities,
        statuses,
        status_codes,
        direction,
        directions,
        reason_codes,
        reason_texts,
    })
}

/// Only explicit acceptance or rejection codes establish an outcome. Other
/// codelist values (for example informational 21G) leave the outcome unknown.
pub fn reason_outcome(codes: &[String]) -> Option<bool> {
    if codes.iter().any(|c| {
        matches!(
            c.as_str(),
            "04G"
                | "12G"
                | "13G"
                | "14G"
                | "15G"
                | "23G"
                | "40G"
                | "41G"
                | "44G"
                | "45G"
                | "46G"
                | "47G"
                | "50G"
                | "61G"
                | "62G"
                | "65G"
                | "66G"
                | "67G"
                | "68G"
                | "69G"
                | "75G"
                | "76G"
                | "77G"
                | "78G"
                | "81G"
                | "91G"
                | "97G"
                | "99G"
        )
    }) {
        Some(false)
    } else if !codes.is_empty()
        && codes.iter().all(|c| {
            matches!(
                c.as_str(),
                "01G" | "02G" | "03G" | "01H" | "84G" | "92G" | "93G" | "95G" | "02H" | "82G"
            )
        })
    {
        Some(true)
    } else {
        None
    }
}

fn parse_nomination_profiles(
    root: Node<'_, '_>,
    hours: &[Hour],
    response: bool,
) -> Result<Vec<NominationProfile>, String> {
    let mut profiles = Vec::new();
    for internal in children(root, "Internal_Account") {
        let internal_account = tag(internal, "internalAccount")?;
        for cp in children(internal, "ConnectionPoint") {
            let point = one(cp, "identification")?;
            let mut external = Vec::new();
            for nt in children(cp, "NominationType") {
                let code = tag(nt, "nominationCode")?.to_string();
                external.extend(
                    children(nt, "External_Account")
                        .into_iter()
                        .map(|n| (Some(code.clone()), n)),
                );
            }
            external.extend(
                children(cp, "External_Account")
                    .into_iter()
                    .map(|n| (None, n)),
            );
            for (nomination_code, ext) in external {
                let counterparty = tag(ext, "externalAccount")?.to_string();
                let (p, counterparty_quantities, counterparty_directions) = if response {
                    let ts = children(ext, "InformationOrigin_TimeSeries");
                    let confirmed = ts
                        .iter()
                        .copied()
                        .filter(|s| tag(*s, "businessCode") == Ok("16G"))
                        .collect::<Vec<_>>();
                    let counter = ts
                        .iter()
                        .copied()
                        .filter(|s| tag(*s, "businessCode") == Ok("18G"))
                        .collect::<Vec<_>>();
                    if confirmed.len() != 1 || counter.len() > 1 || ts.len() != 1 + counter.len() {
                        return Err(
                            "NOMRES requires one confirmed 16G series and at most one 18G series"
                                .into(),
                        );
                    }
                    let p = profile(confirmed[0], hours)?;
                    let (counterparty_quantities, counterparty_directions) =
                        if let Some(c) = counter.first() {
                            let counter = profile(*c, hours)?;
                            // 18G describes the original counterpart submission,
                            // including a same-side nomination that did not match.
                            (counter.quantities, counter.directions)
                        } else {
                            (vec![], vec![])
                        };
                    (p, counterparty_quantities, counterparty_directions)
                } else {
                    (profile(ext, hours)?, vec![], vec![])
                };

                profiles.push(NominationProfile {
                    internal_account: internal_account.into(),
                    point: point.text().ok_or("Empty connection point")?.into(),
                    point_scheme: point.attribute("codingScheme").unwrap_or("").into(),
                    nomination_code,
                    counterparty,
                    direction: p.direction,
                    directions: p.directions,
                    quantities: p.quantities,
                    statuses: p.statuses,
                    status_codes: p.status_codes,
                    counterparty_quantities,
                    counterparty_directions,
                });
            }
        }
    }
    if profiles.is_empty() {
        return Err("At least one nomination profile is required".into());
    }
    Ok(profiles)
}

fn parse_balancing(root: Node<'_, '_>, hours: &[Hour]) -> Result<BalancingDetail, String> {
    let area = one(root, "MarketArea")?;
    let point = one(area, "identification")?;
    if !((point.attribute("codingScheme") == Some("ZSO") && point.text() == Some("BVP"))
        || (point.attribute("codingScheme") == Some("305")
            && point.text() == Some("21Y000000000014L")))
    {
        return Err("MARSIT requires BVP market area".into());
    }
    let account = one(area, "Account")?;
    let account_id = one(account, "identification")?;
    if account_id.text() != Some(BRP_ACCOUNT)
        || account_id.attribute("codingScheme") != Some("ZSO")
        || tag(account, "accountCode")? != "ZOE"
    {
        return Err("MARSIT requires the synthetic BRP ZOE portfolio".into());
    }
    let series = one(account, "TimeSeries")?;
    if children(root, "ConnectionPoint").len() != 0
        || area.children().filter(|n| n.is_element()).count() != 2
        || account.children().filter(|n| n.is_element()).count() != 3
        || tag(series, "businessCode")? != "Z01"
        || tag(series, "measureUnit.unitOfMeasureCode")? != "KW1"
        || tag(series, "currency.currencyCode")? != "EUR"
        || !children(series, "Period").is_empty()
    {
        return Err("MARSIT requires one allocated Z01/KW1/EUR transaction series".into());
    }
    let mut transactions = Vec::new();
    let mut coverage = vec![false; hours.len()];
    let mut no_transaction = vec![false; hours.len()];
    let mut allocated = vec![false; hours.len()];
    let mut used = std::collections::HashSet::new();
    for transaction in children(series, "Transaction") {
        let identification = tag(transaction, "identification")?.to_string();
        if identification != "NO_TRANS"
            && !(identification.len() == 8
                && identification.starts_with("TR")
                && identification[2..].bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("Transaction identification must be TR plus six digits or NO_TRANS".into());
        }
        if transaction.children().filter(|n| n.is_element()).count() != 2 {
            return Err("MARSIT transaction contains unsupported fields".into());
        }
        let period = one(transaction, "Period")?;
        if period.children().filter(|n| n.is_element()).count() != 5
            || tag(period, "status.statusCode")? != "05G"
        {
            return Err("MARSIT requires definitive 05G quantity/direction/price periods".into());
        }
        let (start, end) = parse_span(tag(period, "timeInterval")?)?;
        let direction = match tag(period, "accountDirection.accountDirectionCode")? {
            "ZPD" => "debit",
            "ZPE" => "credit",
            _ => return Err("MARSIT account direction must be ZPD or ZPE".into()),
        };
        let raw = tag(period, "accountDirection.account_Quantity.amount")?;
        let quantity = raw
            .parse::<i64>()
            .map_err(|_| "MARSIT quantity must be a natural integer")?;
        if raw.is_empty()
            || !raw.bytes().all(|b| b.is_ascii_digit())
            || !(0..=1_000_000_000).contains(&quantity)
        {
            return Err("MARSIT quantity must be bounded nonnegative integer kWh/h".into());
        }
        let price = tag(period, "price.amount")?.to_string();
        let unsigned = price.strip_prefix('-').unwrap_or(&price);
        let valid_price = unsigned.split_once('.').is_some_and(|(whole, fraction)| {
            !whole.is_empty()
                && whole.len() <= 12
                && whole.bytes().all(|b| b.is_ascii_digit())
                && fraction.len() == 6
                && fraction.bytes().all(|b| b.is_ascii_digit())
        });
        if !valid_price {
            return Err("MARSIT price must be EUR/kWh with exactly six decimals".into());
        }
        if identification == "NO_TRANS"
            && (quantity != 0 || unsigned.bytes().any(|b| b != b'0' && b != b'.'))
        {
            return Err("NO_TRANS must have zero quantity and zero price".into());
        }
        let mut count = 0;
        for (i, hour) in hours.iter().enumerate() {
            if parse_time(&hour.start)? >= start && parse_time(&hour.end)? <= end {
                if !used.insert((identification.clone(), i)) {
                    return Err("A transaction may not allocate the same hour twice".into());
                }
                coverage[i] = true;
                if identification == "NO_TRANS" {
                    no_transaction[i] = true;
                } else {
                    allocated[i] = true;
                }
                count += 1;
            }
        }
        if count == 0
            || start < parse_time(&hours[0].start)?
            || end > parse_time(&hours.last().unwrap().end)?
            || (end - start).num_seconds() != count * 3600
        {
            return Err("MARSIT periods must cover whole hours within the gas day".into());
        }
        transactions.push(BalancingTransaction {
            identification,
            periods: vec![BalancingPeriod {
                start: iso(start),
                end: iso(end),
                direction: direction.into(),
                quantity,
                price,
            }],
        });
    }
    if transactions.is_empty() || coverage.iter().any(|covered| !covered) {
        return Err(
            "MARSIT must cover the gas day; use NO_TRANS for hours without transactions".into(),
        );
    }
    if no_transaction
        .iter()
        .zip(allocated)
        .any(|(none, actual)| *none && actual)
    {
        return Err("NO_TRANS may only cover hours without an actual transaction".into());
    }
    Ok(BalancingDetail {
        document_code: tag(root, "documentCode")?.into(),
        portfolio: BRP_ACCOUNT.into(),
        market_area: "BVP".into(),
        unit: "KW1".into(),
        currency: "EUR".into(),
        price_unit: "EUR/kWh".into(),
        transactions,
    })
}

pub fn generate_balancing(input: &BalancingInput) -> Result<String, String> {
    if !matches!(input.document_code.as_str(), "ANZ" | "AVI") {
        return Err("MARSIT document code must be ANZ or AVI".into());
    }
    let id = input
        .document_id
        .as_deref()
        .ok_or("Backend document_id is required")?;
    let f = family("MARSIT")?;
    let mut x = header(f, id, input.version, Some(&input.gas_day), true)?;
    x = x.replace(
        "<documentCode>ANZ</documentCode>",
        &el("documentCode", &input.document_code).trim_end(),
    );
    x += "<MarketArea>\n";
    x += &coded("identification", "BVP", "ZSO");
    x += "<Account>\n";
    x += &coded("identification", BRP_ACCOUNT, "ZSO");
    x += "<accountCode>ZOE</accountCode><TimeSeries><businessCode>Z01</businessCode><measureUnit.unitOfMeasureCode>KW1</measureUnit.unitOfMeasureCode><currency.currencyCode>EUR</currency.currencyCode>\n";
    for transaction in &input.transactions {
        if transaction.periods.len() != 1 {
            return Err("Edigas MARSIT requires exactly one period per Transaction".into());
        }
        x += "<Transaction>";
        x += &el("identification", &transaction.identification);
        let p = &transaction.periods[0];
        x += "<Period>";
        x += &el(
            "timeInterval",
            &format!("{}/{}", xml_time(&p.start), xml_time(&p.end)),
        );
        x += "<status.statusCode>05G</status.statusCode>";
        x += &el(
            "accountDirection.accountDirectionCode",
            match p.direction.as_str() {
                "debit" => "ZPD",
                "credit" => "ZPE",
                _ => return Err("Direction must be debit or credit".into()),
            },
        );
        x += &el(
            "accountDirection.account_Quantity.amount",
            &p.quantity.to_string(),
        );
        x += &el("price.amount", &p.price);
        x += "</Period></Transaction>\n";
    }
    x += "</TimeSeries></Account></MarketArea></MarketSituation_Document>\n";
    let parsed = parse_document(&x)?;
    local_rules(&x, &parsed)?;
    Ok(x)
}

pub fn generate_default_programme(day: &str, sequence: u32) -> Result<String, String> {
    let f = family("PROCON")?;
    let id = format!("RES{}", Uuid::new_v4().simple());
    let mut x = header(f, &id, sequence, Some(day), true)?;
    x += "<proDoc_Document.identification>DEFAULT</proDoc_Document.identification><proDoc_Document.version>1</proDoc_Document.version>\n";
    x += &programme_open();
    let hours = intervals(day)?;
    let zero = vec![0; hours.len()];
    for (name, code, direction) in [
        ("GSTPTRADE", "ZUD", "Z03"),
        ("GSTPD", "ZTX", "Z03"),
        ("GSTPVPPVEN", "ZUD", "Z03"),
        ("GSTPVPPVEX", "ZUD", "Z03"),
    ] {
        x += &account(name, code, &hours, &zero, direction, true);
    }
    x += "</ConnectionPoint></LoadForecastConfirmation_Document>\n";
    Ok(x)
}

pub fn parse_document(xml: &str) -> Result<BusinessDocument, String> {
    let doc = safe_parse(xml)?;
    let root = doc.root_element();
    let f = detect(root)?;
    let id = tag(root, "identification")?.to_string();
    ident(&id)?;
    let creation_date_time = tag(root, "creationDateTime")?.to_string();
    if !creation_date_time.ends_with('Z') {
        return Err("creationDateTime must use UTC Z".into());
    }
    parse_time(&creation_date_time)?;
    let mut b = BusinessDocument {
        creation_date_time,
        nomination_profiles: vec![],
        programme_profiles: vec![],
        id,
        version: ver(tag(root, "version")?)?,
        code: tag(root, "documentCode")?.into(),
        kind: f.kind.into(),
        gas_day: String::new(),
        issuer: tag(root, "issuer_MarketParticipant.identification")?.into(),
        recipient: tag(root, "recipient_MarketParticipant.identification")?.into(),
        reference: None,
        direction: String::new(),
        quantities: vec![],
        counterparty: COUNTERPARTY_ACCOUNT.into(),
        statuses: vec![],
        confirmed: vec![],
        counterparty_quantities: vec![],
        accepted: None,
        reasons: vec![],
        balancing: None,
    };
    let ref_prefix = match f.kind {
        "ACKNOW" => Some("receiving_Document"),
        "NOMRES" => Some("nomination_Document"),
        "PROCON" => Some("proDoc_Document"),
        _ => None,
    };
    if let Some(prefix) = ref_prefix {
        let id = tag(root, &format!("{prefix}.identification"))?.to_string();
        ident(&id)?;
        let version = ver(tag(root, &format!("{prefix}.version"))?)?;
        if f.kind == "NOMRES" && id == "MISSING" && version != 1 {
            return Err("MISSING nomination reference must have version 1".into());
        }
        b.reference = Some(DocumentRef { id, version });
    }
    if f.kind == "ACKNOW" {
        let r = children(root, "Reason");
        if r.is_empty() {
            return Err("ACKNOW requires a reason".into());
        }
        let codes = r
            .iter()
            .filter_map(|r| tag(*r, "reasonCode").ok().map(str::to_owned))
            .collect::<Vec<_>>();
        b.accepted = reason_outcome(&codes);
        b.reasons = r
            .iter()
            .map(|r| {
                let code = tag(*r, "reasonCode").unwrap_or("");
                match tag(*r, "text") {
                    Ok(text) if !code.is_empty() => format!("{code}: {text}"),
                    Ok(text) => text.into(),
                    Err(_) => code.into(),
                }
            })
            .collect();
        return Ok(b);
    }
    b.gas_day = gas_day(tag(root, "validityPeriod")?)?;
    let h = intervals(&b.gas_day)?;
    if f.kind == "MARSIT" {
        b.balancing = Some(parse_balancing(root, &h)?);
        b.counterparty = BRP_ACCOUNT.into();
        return Ok(b);
    }
    if f.kind == "NOMINT" || f.kind == "NOMRES" {
        b.nomination_profiles = parse_nomination_profiles(root, &h, f.kind == "NOMRES")?;
        // Backward compatible projection only. Callers must select a profile
        // explicitly before matching; opposite sides must never be netted here.
        let first = &b.nomination_profiles[0];
        b.counterparty = first.counterparty.clone();
        b.quantities = first.quantities.clone();
        b.direction = first.direction.clone();
        b.statuses = first.statuses.clone();
        b.counterparty_quantities = first.counterparty_quantities.clone();
        if f.kind == "NOMRES" {
            b.confirmed = first.quantities.clone();
        }
    } else {
        for cp in children(root, "ConnectionPoint") {
            for a in children(cp, "Account") {
                let p = profile(a, &h)?;
                b.programme_profiles.push(ProgrammeProfile {
                    account: tag(a, "identification")?.into(),
                    account_code: tag(a, "accountCode")?.into(),
                    quantities: p.quantities,
                    reason_codes: p.reason_codes,
                    reason_texts: p.reason_texts,
                });
            }
        }
        let trade = b
            .programme_profiles
            .iter()
            .filter(|p| p.account == "GSTPTRADE")
            .collect::<Vec<_>>();
        if trade.len() != 1 {
            return Err("Programme requires GSTPTRADE account".into());
        }
        b.quantities = trade[0].quantities.clone();
        if f.kind == "PROCON" {
            b.confirmed = b.quantities.clone();
            b.statuses = trade[0]
                .reason_codes
                .iter()
                .map(|r| r.first().cloned().unwrap_or_default())
                .collect();
            let codes = b
                .programme_profiles
                .iter()
                .flat_map(|p| p.reason_codes.iter().flatten())
                .cloned()
                .collect::<Vec<_>>();
            b.accepted = match reason_outcome(&codes) {
                Some(true)
                    if b.programme_profiles
                        .iter()
                        .any(|p| p.reason_codes.iter().any(Vec::is_empty)) =>
                {
                    None
                }
                outcome => outcome,
            };
            for p in &b.programme_profiles {
                for (codes, texts) in p.reason_codes.iter().zip(&p.reason_texts) {
                    for (code, text) in codes.iter().zip(texts) {
                        let reason = if text.is_empty() {
                            code.clone()
                        } else {
                            format!("{code}: {text}")
                        };
                        if !b.reasons.contains(&reason) {
                            b.reasons.push(reason);
                        }
                    }
                }
            }
        }
        // Programme directions do not establish a nomination side, but retain
        // the legacy first account direction for response generation.
        let cp = one(root, "ConnectionPoint")?;
        let account = children(cp, "Account")
            .into_iter()
            .find(|a| tag(*a, "identification") == Ok("GSTPTRADE"))
            .ok_or("Programme requires GSTPTRADE account")?;
        b.direction = profile(account, &h)?.direction;
    }
    Ok(b)
}

fn local_rules(xml: &str, b: &BusinessDocument) -> Result<(), String> {
    let d = safe_parse(xml)?;
    let r = d.root_element();
    let response = matches!(b.kind.as_str(), "ACKNOW" | "NOMRES" | "PROCON" | "MARSIT");
    let programme = b.kind.starts_with("PRO");
    if r.attribute("schemaVersion") != Some("1") {
        return Err("Edigas 6.1 schemaVersion must be 1".into());
    }
    for node in r.descendants().filter(|n| n.is_element()) {
        let name = node.tag_name().name();
        let scheme = if matches!(
            name,
            "issuer_MarketParticipant.identification"
                | "recipient_MarketParticipant.identification"
        ) {
            Some("305")
        } else if matches!(name, "internalAccount" | "externalAccount")
            || (programme
                && name == "identification"
                && node
                    .parent()
                    .is_some_and(|p| p.tag_name().name() == "Account"))
        {
            Some("ZSO")
        } else {
            None
        };
        if scheme.is_some_and(|s| node.attribute("codingScheme") != Some(s)) {
            return Err(format!("Unexpected codingScheme on {name}"));
        }
    }
    if b.issuer != if response { TSO_EIC } else { BRP_EIC }
        || b.recipient != if response { BRP_EIC } else { TSO_EIC }
    {
        return Err("Only synthetic local BRP/TSO identities are supported".into());
    }
    let issuer_role = if programme {
        "issuer_MarketParticipant.issuer_MarketRole.roleCode"
    } else {
        "issuer_MarketParticipant.marketRole.roleCode"
    };
    let recipient_role = if programme {
        "recipient_MarketParticipant.recipient_MarketRole.roleCode"
    } else {
        "recipient_MarketParticipant.marketRole.roleCode"
    };
    let issuer = tag(r, issuer_role)?;
    let recipient = tag(r, recipient_role)?;
    let tso_role = if programme { "ZSO" } else { "ZUK" };
    // Normative VTP MIG 3.3 and GTS XML examples use ZUK. The GTS
    // implementation table also documents ZSO for NOMRES, so accept that
    // inbound compatibility variant while always generating normative ZUK.
    let issuer_ok = if b.kind == "NOMRES" || b.kind == "ACKNOW" {
        matches!(issuer, "ZUK" | "ZSO")
    } else {
        issuer == if response { tso_role } else { "ZSH" }
    };
    if !issuer_ok || recipient != if response { "ZSH" } else { tso_role } {
        return Err("Unexpected market participant role for this document family".into());
    }
    if b.kind == "MARSIT" {
        return Ok(()); // parse_balancing enforces the bounded business profile.
    }
    if b.kind == "ACKNOW" {
        if !matches!(tag(r, "receiving_Document.documentCode")?, "02G" | "ALH") {
            return Err("ACKNOW reference must be a local submission".into());
        }
        return Ok(());
    }
    if !programme {
        let referenced_code = if b.kind == "NOMRES" {
            let code = tag(r, "nomination_Document.documentCode")?;
            if !matches!(code, "02G" | "03G") {
                return Err("NOMRES must reference VTP OTC 02G or exchange 03G".into());
            }
            code
        } else {
            "02G"
        };
        let mut keys = std::collections::HashSet::new();
        for internal in children(r, "Internal_Account") {
            if tag(internal, "internalAccount")? != BRP_ACCOUNT {
                return Err("Unexpected internal portfolio".into());
            }
            for cp in children(internal, "ConnectionPoint") {
                if tag(cp, "measureUnit.unitOfMeasureCode")? != "KW1" {
                    return Err("Only KW1 (kWh/h) is supported".into());
                }
            }
        }
        for p in &b.nomination_profiles {
            let aggregate =
                p.point_scheme == "ZSO" && matches!(p.point.as_str(), "GTSTRZ" | "BBLTRZ");
            let ttf = (p.point_scheme == "ZSO" && p.point == "TTF")
                || (p.point_scheme == "305" && p.point == "21YNL----TTF---1");
            if !ttf && !(b.kind == "NOMRES" && aggregate) {
                return Err("Unexpected connection point".into());
            }
            let valid_type = match p.nomination_code.as_deref() {
                Some("A02") => referenced_code == "02G",
                None => referenced_code == "03G" || aggregate,
                _ => false,
            };
            if !valid_type {
                return Err("Only A02 OTC or exchange/aggregate profiles without NominationType are supported".into());
            }
            if p.counterparty == "END USER" {
                if !aggregate {
                    return Err("END USER is reserved for aggregate confirmations".into());
                }
            } else {
                ident(&p.counterparty).map_err(|_| "Invalid counterportfolio identification")?;
            }
            if !keys.insert((
                &p.internal_account,
                &p.point,
                &p.nomination_code,
                &p.counterparty,
            )) {
                return Err("Duplicate nomination counterportfolio profile".into());
            }
        }
        return Ok(());
    }
    let cp = one(r, "ConnectionPoint")?;
    let point = one(cp, "identification")?;
    if !((point.attribute("codingScheme") == Some("ZSO") && point.text() == Some("VPPV"))
        || (point.attribute("codingScheme") == Some("305")
            && point.text() == Some("21Y000000000011R")))
    {
        return Err("Unexpected connection point".into());
    }
    if tag(cp, "measureUnit.unitOfMeasureCode")? != "KW1" {
        return Err("Only KW1 (kWh/h) is supported".into());
    }
    if b.programme_profiles
        .iter()
        .any(|p| p.quantities.iter().any(|q| *q != 0))
    {
        return Err("Trade programme profiles must be zero".into());
    }
    if programme {
        if tag(r, "contract_Reference.identification")? != BRP_ACCOUNT
            || tag(r, "contract_Reference.referenceCode")? != if response { "CT" } else { "ZSC" }
        {
            return Err("Unexpected programme contract reference".into());
        }
        let a = children(cp, "Account");
        if b.kind == "PRODOC" {
            if a.len() != 1 || tag(a[0], "accountCode")? != "ZOC" {
                return Err("PRODOC requires only GSTPTRADE/ZOC".into());
            }
        } else {
            if a.len() != 4 {
                return Err("PROCON requires all four reserved zero portfolios".into());
            }
            for (name, code) in [
                ("GSTPTRADE", "ZUD"),
                ("GSTPD", "ZTX"),
                ("GSTPVPPVEN", "ZUD"),
                ("GSTPVPPVEX", "ZUD"),
            ] {
                let selected = a
                    .iter()
                    .filter(|n| tag(**n, "identification") == Ok(name))
                    .collect::<Vec<_>>();
                if selected.len() != 1 || tag(*selected[0], "accountCode")? != code {
                    return Err("Unexpected programme confirmation portfolios".into());
                }
            }
        }
    }
    Ok(())
}

fn pinned_bundle(dir: &Path) -> Result<(), String> {
    let canonical = dir
        .canonicalize()
        .map_err(|_| "Pinned schema directory unavailable")?;
    for line in include_str!("../schemas/SHA256SUMS").lines() {
        let (hash, relative) = line
            .split_once("  ")
            .ok_or("Invalid embedded schema manifest")?;
        let p = dir.join(relative);
        let resolved = p
            .canonicalize()
            .map_err(|_| format!("Missing pinned schema {relative}"))?;
        if !resolved.starts_with(&canonical) {
            return Err("Schema bundle symlink escapes its directory".into());
        }
        let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
        if hex::encode(Sha256::digest(bytes)) != hash {
            return Err(format!("Pinned schema checksum mismatch: {relative}"));
        }
    }
    Ok(())
}
pub fn validate_xml(xml: &str, schema_dir: &Path) -> ValidationReport {
    let mut report = ValidationReport {
        valid: false,
        checks: vec![],
        errors: vec![],
    };
    let mut check = |layer: &str, result: Result<String, String>| match result {
        Ok(detail) => report.checks.push(ValidationCheck {
            layer: layer.into(),
            status: "pass".into(),
            detail,
        }),
        Err(e) => {
            report.checks.push(ValidationCheck {
                layer: layer.into(),
                status: "fail".into(),
                detail: e.clone(),
            });
            report.errors.push(e);
        }
    };
    let parsed = safe_parse(xml);
    check(
        "XML security and well-formedness",
        parsed
            .as_ref()
            .map(|_| "Size limit, no DTD/entity declarations, well-formed XML".into())
            .map_err(Clone::clone),
    );
    let detected = parsed
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|d| detect(d.root_element()));
    check(
        "Edigas family",
        detected
            .as_ref()
            .map(|_| "Exact Edigas 6.1 QName and documentCode".into())
            .map_err(Clone::clone),
    );
    let xsd = detected.and_then(|f| {
        pinned_bundle(schema_dir)?;
        let folder = match f.area {
            "General" => "6. General",
            "BalancingAndSettlement" => "4. Balancing and Settlement",
            _ => "3. Nomination and Matching",
        };
        let path = schema_dir.join(folder).join(format!(
            "urn-easee-gas-eu-edigas-{}-{}-6-1.xsd",
            f.area, f.name
        ));
        let mut child = Command::new("xmllint")
            .args(["--nonet", "--nocatalogs", "--noout", "--schema"])
            .arg(path)
            .arg("-")
            .env_remove("XML_CATALOG_FILES")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot run pinned XSD validator: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("Validator input unavailable")?
            .write_all(xml.as_bytes())
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!("XSD: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(
            "Supplied pinned XSD/codelists; offline --nonet; instance schemaLocation ignored"
                .into(),
        )
    });
    check("Pinned XSD", xsd);
    let profile = parse_document(xml).and_then(|b| {
        local_rules(xml, &b)?;
        let compatibility = b.kind == "NOMRES" && safe_parse(xml).ok().is_some_and(|d|
            tag(d.root_element(), "issuer_MarketParticipant.marketRole.roleCode") == Ok("ZSO"));
        Ok(if compatibility {
            "Implemented local GTS subset; inbound NOMRES ZSO compatibility variant from GTS table accepted; normative VTP MIG/example role is ZUK".into()
        } else {
            "Implemented local GTS subset: synthetic parties; normative VTP ZUK roles; TTF/exchange/known aggregate counterportfolio profiles or zero trade programme; KW1, natural quantities and full per-profile hourly coverage; raw supplied status/reason codes preserved".into()
        })
    });
    check("Local profile subset", profile);
    report.checks.push(ValidationCheck{layer:"Business context".into(),status:"not_evaluated".into(),detail:"Standalone XML does not establish registration, contract/capacity, version chronology, counterpart receipt, deadlines, matching truth or production GTS compliance".into()});
    report.valid = report.errors.is_empty();
    report
}
