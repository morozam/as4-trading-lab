//! Independent, hand-authored peer documents exercise the receiver, not its own generator.
use nomination_lab::{
    app::{App, Config},
    edigas::{self, DraftInput},
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
fn node(role: &str) -> (tempfile::TempDir, Arc<App>) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::local(role, dir.path().join("app"));
    cfg.schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let app = App::open(cfg).unwrap();
    (dir, app)
}
fn submit(brp: &App, kind: &str) -> (String, String) {
    let d = brp
        .create_draft(DraftInput {
            kind: kind.into(),
            gas_day: "2026-11-03".into(),
            counterparty: edigas::COUNTERPARTY_ACCOUNT.into(),
            direction: "sell".into(),
            quantities: vec![if kind == "PRODOC" { 0 } else { 100 }; 24],
            document_id: None,
            version: 1,
        })
        .unwrap();
    let view = brp.preview(&d.id).unwrap();
    let id = brp
        .send_draft(&d.id, &view.hash, &format!("send-{kind}"))
        .unwrap();
    (id, view.xml)
}
fn ext(counter: &str, direction: &str, quantity: i64, status: Option<&str>) -> String {
    format!(
        r#"<External_Account><externalAccount codingScheme="ZSO">{counter}</externalAccount><InformationOrigin_TimeSeries><businessCode>16G</businessCode><Period><timeInterval>2026-11-03T05:00Z/2026-11-04T05:00Z</timeInterval><direction.gasDirectionCode>{direction}</direction.gasDirectionCode><quantity.amount>{quantity}</quantity.amount>{}</Period></InformationOrigin_TimeSeries></External_Account>"#,
        status
            .map(|s| format!("<Status><statusCode>{s}</statusCode></Status>"))
            .unwrap_or_default()
    )
}
fn nomres(id: &str, version: u32, reference: &str, point: &str, accounts: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><NominationResponse_Document xmlns="urn:easee-gas.eu:edigas:BrpNominationAndMatching:NominationResponseDocument:6:1" schemaVersion="1"><identification>{id}</identification><version>{version}</version><documentCode>08G</documentCode><creationDateTime>2026-11-02T12:30:00Z</creationDateTime><validityPeriod>2026-11-03T05:00Z/2026-11-04T05:00Z</validityPeriod><issuer_MarketParticipant.identification codingScheme="305">99X-LAB-TSO----0</issuer_MarketParticipant.identification><issuer_MarketParticipant.marketRole.roleCode>ZUK</issuer_MarketParticipant.marketRole.roleCode><recipient_MarketParticipant.identification codingScheme="305">99X-LAB-BRP----0</recipient_MarketParticipant.identification><recipient_MarketParticipant.marketRole.roleCode>ZSH</recipient_MarketParticipant.marketRole.roleCode><nomination_Document.identification>{reference}</nomination_Document.identification><nomination_Document.version>1</nomination_Document.version><nomination_Document.documentCode>02G</nomination_Document.documentCode><Internal_Account><internalAccount codingScheme="ZSO">GSLABBRP</internalAccount><ConnectionPoint><identification codingScheme="ZSO">{point}</identification><measureUnit.unitOfMeasureCode>KW1</measureUnit.unitOfMeasureCode><NominationType><nominationCode>A02</nominationCode>{accounts}</NominationType></ConnectionPoint></Internal_Account></NominationResponse_Document>"#
    )
}
fn procon(id: &str, reference: &str, reason: &str) -> String {
    let mut x = format!(
        r#"<LoadForecastConfirmation_Document xmlns="urn:easee-gas.eu:edigas:General:LoadForecastConfirmationDocument:6:1" schemaVersion="1"><identification>{id}</identification><version>1</version><documentCode>ALK</documentCode><creationDateTime>2026-11-02T14:05:00Z</creationDateTime><validityPeriod>2026-11-03T05:00Z/2026-11-04T05:00Z</validityPeriod><contract_Reference.identification>GSLABBRP</contract_Reference.identification><contract_Reference.referenceCode>CT</contract_Reference.referenceCode><issuer_MarketParticipant.identification codingScheme="305">99X-LAB-TSO----0</issuer_MarketParticipant.identification><issuer_MarketParticipant.issuer_MarketRole.roleCode>ZSO</issuer_MarketParticipant.issuer_MarketRole.roleCode><recipient_MarketParticipant.identification codingScheme="305">99X-LAB-BRP----0</recipient_MarketParticipant.identification><recipient_MarketParticipant.recipient_MarketRole.roleCode>ZSH</recipient_MarketParticipant.recipient_MarketRole.roleCode><proDoc_Document.identification>{reference}</proDoc_Document.identification><proDoc_Document.version>1</proDoc_Document.version><ConnectionPoint><identification codingScheme="ZSO">VPPV</identification><measureUnit.unitOfMeasureCode>KW1</measureUnit.unitOfMeasureCode>"#
    );
    for (a, c) in [
        ("GSTPTRADE", "ZUD"),
        ("GSTPD", "ZTX"),
        ("GSTPVPPVEN", "ZUD"),
        ("GSTPVPPVEX", "ZUD"),
    ] {
        x += &format!(
            r#"<Account><identification codingScheme="ZSO">{a}</identification><accountCode>{c}</accountCode><Period><timeInterval>2026-11-03T05:00Z/2026-11-04T05:00Z</timeInterval><direction.gasDirectionCode>Z03</direction.gasDirectionCode><quantity.amount>0</quantity.amount><Reason><reasonCode>{reason}</reasonCode><text>Actual peer reason</text></Reason></Period></Account>"#
        )
    }
    x += "</ConnectionPoint></LoadForecastConfirmation_Document>";
    x
}
#[test]
fn consolidated_confirmation_preserves_all_counterparties_without_inventing_status() {
    let (_d, brp) = node("brp");
    let (pid, xml) = submit(&brp, "NOMINT");
    let req = edigas::parse_document(&xml).unwrap();
    let xml = nomres(
        "PEER-CONSOLIDATED",
        1,
        &req.id,
        "TTF",
        &(ext("GSLABCOUNTER", "Z03", 100, None) + &ext("GSEXCHANGE", "Z02", 200, None)),
    );
    assert!(brp
        .receive_business("consolidated", json!({}), &xml)
        .unwrap()
        .is_some());
    let state = brp.state().unwrap();
    let p = state.processes.iter().find(|p| p.id == pid).unwrap();
    assert_eq!(p.confirmed, vec![100; 24]);
    assert_ne!(p.stage, "Settled");
    assert!(p.matching_statuses.iter().all(|s| s.is_empty()));
    let record = state.processes.iter().find(|p| p.kind == "NOMRES").unwrap();
    let value = serde_json::to_value(record).unwrap();
    assert_eq!(value["nomination_profiles"].as_array().unwrap().len(), 2);
}
#[test]
fn unsolicited_exchange_confirmation_creates_real_process_without_fabricated_submission() {
    let (_d, brp) = node("brp");
    let xml = nomres(
        "PEER-EXCHANGE",
        1,
        "MISSING",
        "TTF",
        &ext("GSEXCHANGE", "Z02", 200, None),
    );
    let id = brp
        .receive_business("exchange-delivery", json!({}), &xml)
        .unwrap()
        .unwrap();
    let state = brp.state().unwrap();
    assert_eq!(state.processes.len(), 1);
    let p = &state.processes[0];
    assert_eq!(p.id, id);
    assert_eq!(p.kind, "NOMRES");
    assert_eq!(p.ack_status, "Not applicable");
    assert_eq!(p.confirmed, vec![200; 24]);
    assert!(state.drafts.is_empty());
}
#[test]
fn aggregate_confirmation_cannot_overwrite_a_ttf_nomination() {
    let (_d, brp) = node("brp");
    let (pid, xml) = submit(&brp, "NOMINT");
    let req = edigas::parse_document(&xml).unwrap();
    let xml = nomres(
        "PEER-AGGREGATE",
        1,
        &req.id,
        "GTSTRZ",
        &ext("END USER", "Z03", 999, None),
    );
    let other = brp
        .receive_business("aggregate-delivery", json!({}), &xml)
        .unwrap()
        .unwrap();
    assert_ne!(other, pid);
    let state = brp.state().unwrap();
    assert!(state
        .processes
        .iter()
        .find(|p| p.id == pid)
        .unwrap()
        .confirmed
        .is_empty());
    assert_eq!(
        state.processes.iter().find(|p| p.id == other).unwrap().kind,
        "NOMRES"
    );
}
#[test]
fn rejected_programme_reason_is_not_turned_into_acceptance() {
    let (_d, brp) = node("brp");
    let (pid, xml) = submit(&brp, "PRODOC");
    let req = edigas::parse_document(&xml).unwrap();
    let xml = procon("PEER-PROGRAMME-REJECTED", &req.id, "91G");
    brp.receive_business("programme-rejected", json!({}), &xml)
        .unwrap();
    let state = brp.state().unwrap();
    let p = state.processes.iter().find(|p| p.id == pid).unwrap();
    assert_eq!(p.stage, "Programme rejected");
    assert!(p.confirmed.is_empty());
    let value = serde_json::to_value(p).unwrap();
    assert_eq!(value["confirmation_accepted"], false);
    assert!(value["confirmation_reasons"].to_string().contains("91G"));
}
#[test]
fn positive_programme_reason_is_preserved_instead_of_replaced() {
    let (_d, brp) = node("brp");
    let (pid, xml) = submit(&brp, "PRODOC");
    let req = edigas::parse_document(&xml).unwrap();
    brp.receive_business(
        "programme-binding",
        json!({}),
        &procon("PEER-BINDING", &req.id, "82G"),
    )
    .unwrap();
    let p = brp
        .state()
        .unwrap()
        .processes
        .into_iter()
        .find(|p| p.id == pid)
        .unwrap();
    assert_eq!(p.stage, "Programme confirmed");
    assert!(p.matching_statuses.iter().all(|s| s == "82G"));
}
#[test]
fn independent_response_identity_does_not_compete_by_version_number() {
    let (_d, brp) = node("brp");
    let (pid, xml) = submit(&brp, "NOMINT");
    let req = edigas::parse_document(&xml).unwrap();
    let first = nomres(
        "PEER-RESPONSE-A",
        5,
        &req.id,
        "TTF",
        &ext("GSLABCOUNTER", "Z03", 100, Some("12G")),
    );
    brp.receive_business("first-response", json!({}), &first)
        .unwrap();
    let other = nomres(
        "PEER-RESPONSE-B",
        1,
        &req.id,
        "TTF",
        &ext("GSLABCOUNTER", "Z03", 75, Some("06G")),
    );
    let id = brp
        .receive_business("different-response", json!({}), &other)
        .unwrap()
        .unwrap();
    assert_ne!(id, pid);
    let state = brp.state().unwrap();
    assert_eq!(
        state
            .processes
            .iter()
            .find(|p| p.id == pid)
            .unwrap()
            .confirmed,
        vec![100; 24]
    );
    assert_eq!(
        state
            .processes
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .confirmed,
        vec![75; 24]
    );
}
#[test]
fn generated_response_identity_is_stable_across_repeated_results_and_renomination() {
    let (_d, brp) = node("brp");
    let (_t, tso) = node("tso");
    let (pid, xml) = submit(&brp, "NOMINT");
    let tid = tso
        .receive_business("nomination-input", json!({}), &xml)
        .unwrap()
        .unwrap();
    tso.acknowledge(&tid, true, "", "ack").unwrap();
    tso.set_fixture(&tid, Some(vec![100; 24])).unwrap();
    let (a, _) = tso.send_result(&tid, "result-one").unwrap();
    tso.set_fixture(&tid, Some(vec![90; 24])).unwrap();
    let (b, _) = tso.send_result(&tid, "result-two").unwrap();
    let a = edigas::parse_document(&tso.xml(&a).unwrap()).unwrap();
    let b = edigas::parse_document(&tso.xml(&b).unwrap()).unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!((a.version, b.version), (1, 2));
    let revision = brp.revise(&pid).unwrap();
    let review = brp.preview(&revision.id).unwrap();
    brp.send_draft(&revision.id, &review.hash, "send-revision")
        .unwrap();
    let tid = tso
        .receive_business("nomination-revision", json!({}), &review.xml)
        .unwrap()
        .unwrap();
    tso.acknowledge(&tid, true, "", "ack-revision").unwrap();
    tso.set_fixture(&tid, Some(vec![100; 24])).unwrap();
    let (c, _) = tso.send_result(&tid, "result-three").unwrap();
    let c = edigas::parse_document(&tso.xml(&c).unwrap()).unwrap();
    assert_eq!(a.id, c.id);
    assert_eq!(c.version, 3);
}
