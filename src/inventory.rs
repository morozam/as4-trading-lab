//! Read-only public gateway configuration evidence. No private-key or callback-secret exports.
use crate::app::{now, Config, Result};
use roxmltree::{Document, Node};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}
fn text(node: Node<'_, '_>, name: &str) -> String {
    child(node, name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .into()
}
fn descendant(node: Node<'_, '_>, name: &str) -> String {
    node.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .into()
}
fn safe_address(s: &str) -> String {
    // Display only a protocol endpoint, never userinfo, query credentials or callback paths.
    let bare = s.split(['?', '#']).next().unwrap_or("");
    if bare.contains('@') || bare.contains("/integrations/") {
        return "Configured (private address redacted)".into();
    }
    bare.into()
}
fn parse_mode(xml: &str, cfg: &Config) -> std::result::Result<Value, String> {
    let doc = Document::parse(xml).map_err(|_| "Invalid P-Mode XML")?;
    let root = doc.root_element();
    let id = text(root, "id");
    let initiator = child(root, "Initiator");
    let responder = child(root, "Responder");
    let aliases: Vec<_> = root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "KeystoreAlias")
        .filter_map(|n| n.text())
        .map(str::to_owned)
        .collect();
    let local_aliases: Vec<_> = root
        .descendants()
        .filter(|n| {
            n.is_element()
                && n.tag_name().name() == "KeystoreAlias"
                && n.attribute("password").is_some()
        })
        .filter_map(|n| n.text())
        .map(str::to_owned)
        .collect();
    let partner_aliases: Vec<_> = root
        .descendants()
        .filter(|n| {
            n.is_element()
                && n.tag_name().name() == "KeystoreAlias"
                && n.attribute("password").is_none()
        })
        .filter_map(|n| n.text())
        .map(str::to_owned)
        .collect();
    let signing = root
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Signing")
        .map(|n| descendant(n, "Algorithm"))
        .unwrap_or_default();
    let encryption = root
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Encryption")
        .map(|n| {
            let a = descendant(n, "Algorithm");
            if a.is_empty() {
                "Configured".into()
            } else {
                a
            }
        })
        .unwrap_or("Not configured".into());
    let service = root
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Service")
        .map(|n| text(n, "name"))
        .unwrap_or_default();
    Ok(
        json!({"id":id,"direction":if id==cfg.pmode_id{"Outbound"}else if id==cfg.inbound_pmode_id{"Inbound"}else{"Not selected"},"initiator":initiator.map(|n|text(n,"PartyId")).unwrap_or_default(),"responder":responder.map(|n|text(n,"PartyId")).unwrap_or_default(),"service":service,"action":descendant(root,"Action"),"agreement":child(root,"Agreement").map(|n|text(n,"name")).unwrap_or_default(),"address":safe_address(&descendant(root,"Address")),"signing":if signing.is_empty(){"Not configured"}else{&signing},"encryption":encryption,"receipt":if descendant(root,"ReplyPattern").is_empty(){if descendant(root,"NotifyReceiptToBusinessApplication")=="true"{"Receipt callback enabled"}else{"See peer configuration"}}else{"Synchronous response"},"signing_aliases":aliases,"local_signing_aliases":local_aliases,"partner_aliases":partner_aliases,"mep":text(root,"mep"),"binding":text(root,"mepBinding")}),
    )
}
fn certificate(pem: &str) -> Option<Value> {
    let mut child = Command::new("openssl")
        .args([
            "x509",
            "-noout",
            "-subject",
            "-issuer",
            "-serial",
            "-dates",
            "-fingerprint",
            "-sha256",
            "-text",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(pem.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let field = |key: &str| {
        output
            .lines()
            .find_map(|l| l.trim().strip_prefix(key))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let extension = |name: &str| {
        let lines: Vec<_> = output.lines().collect();
        lines
            .iter()
            .position(|line| line.trim().starts_with(name))
            .and_then(|i| lines.get(i + 1))
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "Extension absent".into())
    };
    let before = field("notBefore=");
    let after = field("notAfter=");
    let parse = |s: &str| {
        chrono::NaiveDateTime::parse_from_str(s, "%b %e %H:%M:%S %Y GMT")
            .ok()
            .map(|d| d.and_utc())
    };
    let status = match (parse(&before), parse(&after)) {
        (Some(b), Some(_a)) if b > chrono::Utc::now() => "Not yet valid",
        (_, Some(a)) if a < chrono::Utc::now() => "Expired",
        (_, Some(a)) if a < chrono::Utc::now() + chrono::Duration::days(30) => "Expiring soon",
        (_, Some(_)) => "Within validity period",
        _ => "Validity unavailable",
    };
    Some(
        json!({"subject":field("subject="),"issuer":field("issuer="),"serial":field("serial="),"not_before":parse(&before).map(|d|d.to_rfc3339()).unwrap_or(before),"not_after":parse(&after).map(|d|d.to_rfc3339()).unwrap_or(after),"fingerprint":field("sha256 Fingerprint="),"key_algorithm":field("Public Key Algorithm:"),"key_usage":extension("X509v3 Key Usage:"),"extended_key_usage":extension("X509v3 Extended Key Usage:"),"basic_constraints":extension("X509v3 Basic Constraints:"),"status":status}),
    )
}
fn store_certificates(
    home: &Path,
    store: Node<'_, '_>,
    purpose: &str,
    pmodes: &[Value],
) -> std::result::Result<Vec<Value>, String> {
    let path = home.join(text(store, "Path"));
    let password = text(store, "Password");
    if !path.is_file() {
        return Err(format!("{} keystore unavailable", store.tag_name().name()));
    }
    let keytool = std::env::var_os("JAVA_HOME")
        .map(|p| std::path::PathBuf::from(p).join("bin/keytool"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| "keytool".into());
    let output = Command::new(keytool)
        .args([
            "-J-Duser.language=en",
            "-J-Duser.country=US",
            "-list",
            "-rfc",
            "-keystore",
        ])
        .arg(path)
        .args(["-storepass:env", "LAB_INVENTORY_STORE_PASS"])
        .env("LAB_INVENTORY_STORE_PASS", password)
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "Certificate reader unavailable".to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{} keystore could not be inspected",
            store.tag_name().name()
        ));
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for entry in listing.split("Alias name: ").skip(1) {
        let alias = entry.lines().next().unwrap_or("").trim();
        let mut chain = Vec::new();
        let mut rest = entry;
        while let Some(start) = rest.find("-----BEGIN CERTIFICATE-----") {
            rest = &rest[start..];
            let Some(end) = rest.find("-----END CERTIFICATE-----") else {
                break;
            };
            let end = end + "-----END CERTIFICATE-----".len();
            if let Some(c) = certificate(&rest[..end]) {
                chain.push(c);
            }
            rest = &rest[end..];
        }
        if let Some(mut cert) = chain.first().cloned() {
            let routes: Vec<_> = pmodes
                .iter()
                .filter(|p| {
                    p[if store.tag_name().name() == "PrivateKeys" {
                        "local_signing_aliases"
                    } else if store.tag_name().name() == "TradingPartnerCertificates" {
                        "partner_aliases"
                    } else {
                        "_no_alias_references"
                    }]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(alias)))
                })
                .map(|p| p["id"].clone())
                .collect();
            cert["alias"] = json!(alias);
            cert["store"] = json!(store.tag_name().name());
            cert["purpose"] = json!(if routes.is_empty()
                && store.tag_name().name() != "TrustedCertificates"
            {
                "Present in store; no P-Mode use"
            } else {
                purpose
            });
            cert["pmodes"] = json!(routes);
            cert["chain_length"] = json!(chain.len());
            cert["chain"] = json!(chain);
            entries.push(cert);
        }
    }
    Ok(entries)
}
pub fn read_inventory(cfg: &Config) -> Result<Value> {
    let home = cfg
        .data_dir
        .parent()
        .unwrap_or(&cfg.data_dir)
        .join("gateway");
    let mut errors = Vec::<String>::new();
    let mut pmodes = Vec::new();
    if let Ok(files) = fs::read_dir(home.join("repository/pmodes")) {
        for file in files.flatten() {
            if file.path().extension().is_some_and(|e| e == "xml") {
                match fs::read_to_string(file.path())
                    .ok()
                    .and_then(|x| parse_mode(&x, cfg).ok())
                {
                    Some(mode) => pmodes.push(mode),
                    None => errors.push("A P-Mode file could not be read".into()),
                }
            }
        }
    } else {
        errors.push("Gateway P-Mode directory unavailable".into());
    }
    pmodes.sort_by_key(|v| v["id"].as_str().unwrap_or("").to_string());
    let mut certificates = Vec::new();
    match fs::read_to_string(home.join("conf/certmanager_config.xml")) {
        Ok(xml) => match Document::parse(&xml) {
            Ok(doc) => {
                for (name, purpose) in [
                    ("PrivateKeys", "Local signing credential"),
                    (
                        "TradingPartnerCertificates",
                        "Partner verification / encryption certificate",
                    ),
                    ("TrustedCertificates", "Configured trust certificate"),
                ] {
                    if let Some(store) = doc
                        .descendants()
                        .find(|n| n.is_element() && n.tag_name().name() == name)
                    {
                        match store_certificates(&home, store, purpose, &pmodes) {
                            Ok(c) => certificates.extend(c),
                            Err(e) => errors.push(e),
                        }
                    }
                }
            }
            Err(_) => errors.push("Certificate configuration XML is invalid".into()),
        },
        Err(_) => errors.push("Certificate configuration unavailable".into()),
    }
    Ok(
        json!({"read_at":now(),"loaded_status":"Configuration on disk; gateway load status not verified","certificates":certificates,"pmodes":pmodes,"errors":errors}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn certificate_usage_is_visible_without_any_private_material() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("public.pem");
        let private = dir.path().join("private.pem");
        let status = Command::new("openssl")
            .args(["req", "-x509", "-newkey", "rsa:2048", "-noenc", "-keyout"])
            .arg(&private)
            .arg("-out")
            .arg(&cert)
            .args([
                "-subj",
                "/CN=Inventory Test",
                "-days",
                "1",
                "-addext",
                "keyUsage=critical,digitalSignature,keyEncipherment",
                "-addext",
                "extendedKeyUsage=serverAuth",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let value = certificate(&fs::read_to_string(cert).unwrap()).unwrap();
        assert!(value["key_usage"]
            .as_str()
            .unwrap_or("")
            .contains("Digital Signature"));
        assert!(value["extended_key_usage"]
            .as_str()
            .unwrap_or("")
            .contains("TLS Web Server Authentication"));
        assert!(!value.to_string().contains("PRIVATE KEY"));
    }
}
