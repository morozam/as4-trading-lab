use nomination_lab::{
    app::{App, Config},
    edigas::{self, DraftInput, MatchingResult},
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};

fn node(role: &str) -> (tempfile::TempDir, Arc<App>) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Config::local(role, dir.path().into());
    c.schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let app = App::open(c).unwrap();
    (dir, app)
}
fn marsit(code: &str, version: u32) -> String {
    let transactions = [
        (
            "NO_TRANS",
            "2026-11-03T05:00Z/2026-11-03T16:00Z",
            "ZPD",
            0,
            "0.000000",
        ),
        (
            "TR123456",
            "2026-11-03T16:00Z/2026-11-04T05:00Z",
            "ZPD",
            200,
            "0.123456",
        ),
        (
            "TR654321",
            "2026-11-03T17:00Z/2026-11-04T05:00Z",
            "ZPE",
            100,
            "-0.020000",
        ),
    ];
    let mut x = format!("<MarketSituation_Document xmlns=\"urn:easee-gas.eu:edigas:BalancingAndSettlement:MarketSituationDocument:6:1\" schemaVersion=\"1\"><identification>MARSIT-ONE</identification><version>{version}</version><documentCode>{code}</documentCode><creationDateTime>2026-11-03T18:22:00Z</creationDateTime><validityPeriod>2026-11-03T05:00Z/2026-11-04T05:00Z</validityPeriod><issuer_MarketParticipant.identification codingScheme=\"305\">{}</issuer_MarketParticipant.identification><issuer_MarketParticipant.marketRole.roleCode>ZUK</issuer_MarketParticipant.marketRole.roleCode><recipient_MarketParticipant.identification codingScheme=\"305\">{}</recipient_MarketParticipant.identification><recipient_MarketParticipant.marketRole.roleCode>ZSH</recipient_MarketParticipant.marketRole.roleCode><MarketArea><identification codingScheme=\"ZSO\">BVP</identification><Account><identification codingScheme=\"ZSO\">{}</identification><accountCode>ZOE</accountCode><TimeSeries><businessCode>Z01</businessCode><measureUnit.unitOfMeasureCode>KW1</measureUnit.unitOfMeasureCode><currency.currencyCode>EUR</currency.currencyCode>", edigas::TSO_EIC, edigas::BRP_EIC, edigas::BRP_ACCOUNT);
    for (id, span, direction, q, price) in transactions {
        x += &format!("<Transaction><identification>{id}</identification><Period><timeInterval>{span}</timeInterval><status.statusCode>05G</status.statusCode><accountDirection.accountDirectionCode>{direction}</accountDirection.accountDirectionCode><accountDirection.account_Quantity.amount>{q}</accountDirection.account_Quantity.amount><price.amount>{price}</price.amount></Period></Transaction>");
    }
    x + "</TimeSeries></Account></MarketArea></MarketSituation_Document>"
}
fn programme(id: &str) -> String {
    let input = DraftInput {
        kind: "PRODOC".into(),
        gas_day: "2026-11-03".into(),
        counterparty: edigas::COUNTERPARTY_ACCOUNT.into(),
        direction: "sell".into(),
        quantities: vec![0; 24],
        document_id: Some(id.into()),
        version: 1,
    };
    let req = edigas::parse_document(&edigas::generate_submission(&input).unwrap()).unwrap();
    edigas::generate_result(
        &req,
        &MatchingResult {
            quantities: vec![0; 24],
            statuses: vec!["12G".into(); 24],
        },
        1,
    )
    .unwrap()
}
#[test]
fn independent_marsit_retains_all_transactions_and_revision_evidence() {
    for code in ["ANZ", "AVI"] {
        let (_dir, brp) = node("brp");
        let xml = marsit(code, 1);
        let report = edigas::validate_xml(&xml, &brp.config.schema_dir);
        assert!(report.valid, "{:?}", report.errors);
        let pid = brp
            .receive_business("marsit-1", json!({}), &xml)
            .unwrap()
            .expect("independent MARSIT process");
        brp.receive_business("marsit-duplicate", json!({}), &xml)
            .unwrap();
        brp.receive_business("marsit-v2", json!({}), &marsit(code, 2))
            .unwrap();
        let state = serde_json::to_value(brp.state().unwrap()).unwrap();
        assert_eq!(state["processes"].as_array().unwrap().len(), 2);
        let p = state["processes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == pid)
            .unwrap();
        assert_eq!(p["balancing"]["transactions"].as_array().unwrap().len(), 3);
        assert_eq!(
            p["balancing"]["transactions"][2]["periods"][0]["price"],
            "-0.020000"
        );
        assert_eq!(brp.state().unwrap().connection.queued, 0);
    }
}
#[test]
fn default_programme_is_independent_and_does_not_fabricate_a_submission() {
    let (_dir, brp) = node("brp");
    let xml = programme("DEFAULT");
    let pid = brp
        .receive_business("default-1", json!({}), &xml)
        .unwrap()
        .expect("default programme process");
    let state = serde_json::to_value(brp.state().unwrap()).unwrap();
    assert_eq!(state["processes"][0]["id"], pid);
    assert_eq!(state["processes"][0]["programme_origin"], "default");
    assert_eq!(state["processes"][0]["kind"], "PROCON");
    assert!(state["drafts"].as_array().unwrap().is_empty());
    assert_eq!(state["messages"].as_array().unwrap().len(), 1);
}
#[test]
fn zero_programme_direction_has_no_business_effect() {
    let (_dir, brp) = node("brp");
    let input = DraftInput {
        kind: "PRODOC".into(),
        gas_day: "2026-11-03".into(),
        counterparty: edigas::COUNTERPARTY_ACCOUNT.into(),
        direction: "buy".into(),
        quantities: vec![0; 24],
        document_id: None,
        version: 1,
    };
    let draft = brp.create_draft(input).unwrap();
    let preview = brp.preview(&draft.id).unwrap();
    brp.send_draft(&draft.id, &preview.hash, "submit-programme")
        .unwrap();
    let req = edigas::parse_document(&preview.xml).unwrap();
    let xml = programme(&req.id);
    brp.receive_business("opposite-zero", json!({}), &xml)
        .unwrap();
    assert_eq!(brp.state().unwrap().processes[0].confirmed, vec![0; 24]);
}

#[test]
fn zero_programme_may_use_both_direction_codes_without_changing_its_zero_profile() {
    let (_dir, brp) = node("brp");
    let xml = programme("DEFAULT").replacen(
        "<direction.gasDirectionCode>Z03</direction.gasDirectionCode>",
        "<direction.gasDirectionCode>Z02</direction.gasDirectionCode>",
        1,
    );
    let report = edigas::validate_xml(&xml, &brp.config.schema_dir);
    assert!(report.valid, "{:?}", report.errors);
    assert!(brp
        .receive_business("mixed-zero", json!({}), &xml)
        .unwrap()
        .is_some());
}

#[test]
fn outbound_allocations_are_reviewed_frozen_and_use_only_the_as4_outbox() {
    let (_dir, tso) = node("tso");
    let (_brp_dir, brp) = node("brp");
    let parsed = edigas::parse_document(&marsit("AVI", 1)).unwrap();
    let input = edigas::BalancingInput {
        gas_day: parsed.gas_day,
        document_code: parsed.code,
        document_id: None,
        version: 1,
        transactions: parsed.balancing.unwrap().transactions,
    };
    assert!(brp.preview_balancing(input.clone()).is_err());
    let preview = tso.preview_balancing(input).unwrap();
    assert!(preview.validation.valid, "{:?}", preview.validation.errors);
    assert!(tso
        .send_balancing(&preview.id, "stale", "allocation-send")
        .is_err());
    let pid = tso
        .send_balancing(&preview.id, &preview.hash, "allocation-send")
        .unwrap();
    assert_eq!(
        pid,
        tso.send_balancing(&preview.id, &preview.hash, "allocation-send")
            .unwrap()
    );
    assert!(tso
        .send_balancing(&preview.id, &preview.hash, "other-command")
        .is_err());
    let state = tso.state().unwrap();
    assert_eq!(state.connection.queued, 1);
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].xml, preview.xml);
    assert_eq!(state.messages[0].hash, preview.hash);
    assert_eq!(state.messages[0].code, "AVI");
    assert!(
        brp.state().unwrap().processes.is_empty(),
        "No application peer shortcut"
    );
    let cfg = tso.config.clone();
    drop(tso);
    let reopened = App::open(cfg).unwrap();
    assert_eq!(reopened.state().unwrap().messages[0].xml, preview.xml);
    let received = brp
        .receive_business("actual-allocation-delivery", json!({}), &preview.xml)
        .unwrap();
    assert!(received.is_some());
}

#[test]
fn default_programme_is_frozen_idempotent_and_uses_no_fabricated_request() {
    let (_dir, tso) = node("tso");
    let (_brp_dir, brp) = node("brp");
    assert!(brp
        .send_default_programme("2026-11-03", "default-send")
        .is_err());
    let pid = tso
        .send_default_programme("2026-11-03", "default-send")
        .unwrap();
    assert_eq!(
        pid,
        tso.send_default_programme("2026-11-03", "default-send")
            .unwrap()
    );
    assert!(tso
        .send_default_programme("2026-11-04", "default-send")
        .is_err());
    assert!(tso
        .send_default_programme("2026-11-03", "new-default-send")
        .is_err());
    let state = tso.state().unwrap();
    assert_eq!(state.processes.len(), 1);
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].family, "PROCON");
    assert_eq!(
        edigas::parse_document(&state.messages[0].xml)
            .unwrap()
            .reference
            .unwrap()
            .id,
        "DEFAULT"
    );
    assert_eq!(state.connection.queued, 1);
    assert!(brp.state().unwrap().processes.is_empty());
    brp.receive_business("default-delivered", json!({}), &state.messages[0].xml)
        .unwrap();
    assert_eq!(brp.state().unwrap().processes[0].confirmed, vec![0; 24]);
}

#[test]
fn marsit_profile_rejects_invalid_units_identity_coverage_prices_and_duplicate_allocations() {
    let (_dir, brp) = node("brp");
    let xml = marsit("ANZ", 1);
    let variants = [
        xml.replace("KW1", "KWH"),
        xml.replace("EUR", "USD"),
        xml.replace(edigas::BRP_ACCOUNT, "OTHER"),
        xml.replace("BVP", "TTF"),
        xml.replace("ZUK", "ZSO"),
        xml.replace(edigas::TSO_EIC, "99X-OTHER-----0"),
        xml.replace("0.123456", "0.12345"),
        xml.replace(
            "<accountDirection.account_Quantity.amount>200<",
            "<accountDirection.account_Quantity.amount>-1<",
        ),
        xml.replace(
            "2026-11-03T05:00Z/2026-11-03T16:00Z",
            "2026-11-03T06:00Z/2026-11-03T16:00Z",
        ),
        xml.replace("TR654321", "TR123456"),
    ];
    for (i, invalid) in variants.into_iter().enumerate() {
        assert!(
            !edigas::validate_xml(&invalid, &brp.config.schema_dir).valid,
            "variant {i}"
        );
        assert!(brp
            .receive_business(&format!("invalid-{i}"), json!({}), &invalid)
            .unwrap()
            .is_none());
    }
    assert!(brp.state().unwrap().processes.is_empty());
}

#[test]
fn message_details_retains_exact_xml_and_excludes_callback_secrets() {
    let (_dir, brp) = node("brp");
    let xml = programme("DEFAULT");
    brp.receive_business("details-delivery",json!({"authorization":"Bearer SECRET", "cookie":"SECRET", "x-private-token":"SECRET", "x-holodeckb2b-pmodeid":"inbound-pmode", "x-holodeckb2b-conversationid":"conversation-public"}),&xml).unwrap();
    let details = brp.message_details("details-delivery").unwrap();
    assert_eq!(details["message"]["xml"], xml);
    assert_eq!(details["business"]["reference"]["id"], "DEFAULT");
    assert_eq!(
        details["callbacks"][0]["metadata"]["x-holodeckb2b-pmodeid"],
        "inbound-pmode"
    );
    assert!(!details.to_string().contains("SECRET"));
    assert!(!details.to_string().contains(&brp.config.callback_token));
    assert!(!details.to_string().contains(&brp.config.password));
    assert_eq!(brp.message_details("missing").unwrap_err().status, 404);
}

#[test]
fn incoming_allocations_and_default_confirmations_cannot_be_revised_as_brp_submissions() {
    let (_dir, brp) = node("brp");
    for (key, xml) in [
        ("incoming-marsit", marsit("ANZ", 1)),
        ("incoming-default", programme("DEFAULT")),
    ] {
        let pid = brp.receive_business(key, json!({}), &xml).unwrap().unwrap();
        assert!(brp.revise(&pid).is_err(), "{key} is not a BRP submission");
    }
    assert!(brp.state().unwrap().drafts.is_empty());
}

#[test]
fn no_transaction_periods_cannot_overlap_actual_allocations() {
    let (_dir, brp) = node("brp");
    let xml = marsit("ANZ", 1).replace(
        "2026-11-03T05:00Z/2026-11-03T16:00Z",
        "2026-11-03T05:00Z/2026-11-04T05:00Z",
    );
    let report = edigas::validate_xml(&xml, &brp.config.schema_dir);
    assert!(
        !report.valid,
        "NO_TRANS contradicts a real allocation in the same hour"
    );
}

#[test]
fn submitted_programme_still_requires_acceptance_and_confirms_via_as4() {
    let (_brp_dir, brp) = node("brp");
    let (_tso_dir, tso) = node("tso");
    let draft = brp
        .create_draft(DraftInput {
            kind: "PRODOC".into(),
            gas_day: "2026-11-03".into(),
            counterparty: edigas::COUNTERPARTY_ACCOUNT.into(),
            direction: "buy".into(),
            quantities: vec![0; 24],
            document_id: None,
            version: 1,
        })
        .unwrap();
    let preview = brp.preview(&draft.id).unwrap();
    let brp_pid = brp
        .send_draft(&draft.id, &preview.hash, "submitted-programme-send")
        .unwrap();
    let tso_pid = tso
        .receive_business("submitted-programme-deliver", json!({}), &preview.xml)
        .unwrap()
        .unwrap();
    assert!(tso.send_result(&tso_pid, "result-before-ack").is_err());
    assert!(tso
        .send_default_programme("2026-11-03", "incorrect-default")
        .is_err());
    let ack = tso
        .acknowledge(&tso_pid, true, "", "programme-ack")
        .unwrap();
    brp.receive_business(
        "programme-ack-delivered",
        json!({}),
        &tso.xml(&ack).unwrap(),
    )
    .unwrap();
    let (result, _) = tso.send_result(&tso_pid, "programme-result").unwrap();
    brp.receive_business(
        "programme-result-delivered",
        json!({}),
        &tso.xml(&result).unwrap(),
    )
    .unwrap();
    let state = brp.state().unwrap();
    let p = state.processes.iter().find(|p| p.id == brp_pid).unwrap();
    assert_eq!(p.stage, "Programme confirmed");
    assert_eq!(p.programme_origin.as_deref(), Some("submitted"));
    assert_eq!(p.confirmed, vec![0; 24]);
    let details = tso.message_details(&result).unwrap();
    assert_eq!(details["outbox"]["status"], "queued");
    assert_eq!(
        details["transport"]["ref_to_message_id"],
        "submitted-programme-deliver"
    );
}

#[test]
fn default_programme_does_not_overwrite_a_confirmed_submission_or_its_ack_state() {
    let (_dir, brp) = node("brp");
    let draft = brp
        .create_draft(DraftInput {
            kind: "PRODOC".into(),
            gas_day: "2026-11-03".into(),
            counterparty: edigas::COUNTERPARTY_ACCOUNT.into(),
            direction: "sell".into(),
            quantities: vec![0; 24],
            document_id: None,
            version: 1,
        })
        .unwrap();
    let preview = brp.preview(&draft.id).unwrap();
    let pid = brp
        .send_draft(&draft.id, &preview.hash, "programme-to-preserve")
        .unwrap();
    let req = edigas::parse_document(&preview.xml).unwrap();
    let ack = edigas::generate_ack(&req, true, "").unwrap();
    brp.receive_business("preserved-ack", json!({}), &ack)
        .unwrap();
    let result = edigas::generate_result(
        &req,
        &MatchingResult {
            quantities: vec![0; 24],
            statuses: vec!["12G".into(); 24],
        },
        1,
    )
    .unwrap();
    brp.receive_business("preserved-result", json!({}), &result)
        .unwrap();
    let before = serde_json::to_value(
        brp.state()
            .unwrap()
            .processes
            .into_iter()
            .find(|p| p.id == pid)
            .unwrap(),
    )
    .unwrap();
    for (version, key) in [(5, "unrelated-higher-default"), (1, "late-older-default")] {
        let xml = edigas::generate_default_programme("2026-11-03", version).unwrap();
        let default_pid = brp.receive_business(key, json!({}), &xml).unwrap().unwrap();
        assert_ne!(
            default_pid, pid,
            "DEFAULT has no causal reference to the submission"
        );
        let state = brp.state().unwrap();
        let after =
            serde_json::to_value(state.processes.iter().find(|p| p.id == pid).unwrap()).unwrap();
        assert_eq!(
            after, before,
            "Unrelated default must preserve the entire submitted process"
        );
    }
    assert_eq!(
        brp.state().unwrap().processes.len(),
        3,
        "Different default document IDs retain separate actual processes"
    );
}

#[test]
fn default_revisions_are_scoped_to_actual_response_identity_and_have_one_active_version() {
    let (_dir, brp) = node("brp");
    let xml = edigas::generate_default_programme("2026-11-03", 2).unwrap();
    let latest = brp
        .receive_business("default-stream-v2", json!({}), &xml)
        .unwrap()
        .unwrap();
    brp.receive_business("default-stream-v2-duplicate", json!({}), &xml)
        .unwrap();
    let older = xml.replacen("<version>2</version>", "<version>1</version>", 1);
    let older_id = brp
        .receive_business("default-stream-v1-late", json!({}), &older)
        .unwrap()
        .unwrap();
    assert_ne!(
        latest, older_id,
        "Every actual version retains its own process evidence"
    );
    let newer = xml.replacen("<version>2</version>", "<version>3</version>", 1);
    let newest = brp
        .receive_business("default-stream-v3", json!({}), &newer)
        .unwrap()
        .unwrap();
    let state = brp.state().unwrap();
    assert_eq!(state.processes.len(), 3);
    assert_eq!(
        state
            .processes
            .iter()
            .filter(|p| p.stage == "Default programme confirmed")
            .count(),
        1
    );
    assert_eq!(
        state
            .processes
            .iter()
            .find(|p| p.id == newest)
            .unwrap()
            .version,
        3
    );
    assert!(state
        .processes
        .iter()
        .filter(|p| p.id != newest)
        .all(|p| p.stage == "Superseded default programme"));
    let other = edigas::generate_default_programme("2026-11-03", 1).unwrap();
    let other_id = brp
        .receive_business("another-default-document", json!({}), &other)
        .unwrap()
        .unwrap();
    assert_ne!(other_id, newest);
    assert_eq!(
        brp.state()
            .unwrap()
            .processes
            .iter()
            .filter(|p| p.stage == "Default programme confirmed")
            .count(),
        2,
        "Different document streams do not compare unrelated version numbers"
    );
}
