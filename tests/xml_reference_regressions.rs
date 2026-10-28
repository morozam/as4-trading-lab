//! Independent MIG/GTS examples, hand composed with synthetic lab identities.
use nomination_lab::edigas::*;
use std::path::Path;

const NOMINT: &str = include_str!("fixtures/reference/nomint_multi.xml");
const NOMRES: &str = include_str!("fixtures/reference/nomres_multi.xml");
const AGGREGATE: &str = include_str!("fixtures/reference/nomres_aggregate.xml");
const EXCHANGE: &str = include_str!("fixtures/reference/nomres_exchange.xml");
const PROCON: &str = include_str!("fixtures/reference/procon_rejected.xml");
fn valid(xml: &str) {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let r = validate_xml(xml, &schema);
    assert!(r.valid, "{:?}", r.errors);
}
fn value(xml: &str) -> serde_json::Value {
    serde_json::to_value(parse_document(xml).expect("reference must parse")).unwrap()
}
#[test]
fn independent_multi_counterparty_nomination_retains_each_side() {
    valid(NOMINT);
    let b = value(NOMINT);
    assert_eq!(b["nomination_profiles"].as_array().unwrap().len(), 2);
    assert_eq!(b["nomination_profiles"][0]["quantities"][0], 125);
    assert_eq!(
        b["nomination_profiles"][1]["counterparty"],
        "GSOTHERCOUNTER"
    );
    assert_eq!(b["nomination_profiles"][1]["direction"], "sell");
    assert_eq!(b["quantities"][0], 125); // Legacy projection is first, never a net sum.
}
#[test]
fn nomres_optional_status_multiple_statuses_and_missing_reference_are_preserved() {
    valid(NOMRES);
    let b = value(NOMRES);
    assert_eq!(b["reference"]["id"], "MISSING");
    assert_eq!(b["nomination_profiles"][0]["statuses"][0], "");
    assert_eq!(
        b["nomination_profiles"][1]["status_codes"][0],
        serde_json::json!(["35G", "37G"])
    );
    assert_eq!(b["nomination_profiles"][1]["statuses"][0], "35G");
    assert!(parse_document(&NOMRES.replace(
        "<nomination_Document.version>1",
        "<nomination_Document.version>2"
    ))
    .is_err());
}
#[test]
fn aggregate_and_exchange_responses_parse_without_a_local_request() {
    for point in ["GTSTRZ", "BBLTRZ"] {
        let x = AGGREGATE.replace("GTSTRZ", point);
        valid(&x);
        assert_eq!(value(&x)["nomination_profiles"][0]["point"], point);
        assert_eq!(
            value(&x)["nomination_profiles"][0]["counterparty"],
            "END USER"
        );
    }
    valid(EXCHANGE);
    assert_eq!(
        value(EXCHANGE)["nomination_profiles"][0]["nomination_code"],
        serde_json::Value::Null
    );
    assert_eq!(value(EXCHANGE)["quantities"][0], 90);
}
#[test]
fn mixed_hour_directions_are_retained_without_netted_projection() {
    let x = AGGREGATE.replace("<Period><timeInterval>2026-10-27T05:00Z/2026-10-28T05:00Z</timeInterval><direction.gasDirectionCode>Z03</direction.gasDirectionCode><quantity.amount>150</quantity.amount></Period>", "<Period><timeInterval>2026-10-27T05:00Z/2026-10-27T17:00Z</timeInterval><direction.gasDirectionCode>Z02</direction.gasDirectionCode><quantity.amount>150</quantity.amount></Period><Period><timeInterval>2026-10-27T17:00Z/2026-10-28T05:00Z</timeInterval><direction.gasDirectionCode>Z03</direction.gasDirectionCode><quantity.amount>200</quantity.amount></Period>");
    valid(&x);
    let b = value(&x);
    assert_eq!(b["direction"], "mixed");
    assert_eq!(b["nomination_profiles"][0]["directions"][0], "buy");
    assert_eq!(b["nomination_profiles"][0]["directions"][23], "sell");
    assert_eq!(b["quantities"][23], 200);
}
#[test]
fn generated_ttf_roles_are_normative_and_ack_references_original_timestamp() {
    let input = DraftInput {
        kind: "NOMINT".into(),
        gas_day: "2026-10-27".into(),
        counterparty: COUNTERPARTY_ACCOUNT.into(),
        direction: "buy".into(),
        quantities: vec![1; 24],
        document_id: Some("GEN-NOMINT".into()),
        version: 1,
    };
    let x = generate_submission(&input).unwrap();
    assert!(x.contains("<recipient_MarketParticipant.marketRole.roleCode>ZUK</recipient_MarketParticipant.marketRole.roleCode>"));
    assert!(x.contains("<internalAccount codingScheme=\"ZSO\">"));
    let request = parse_document(NOMINT).unwrap();
    let ack = generate_ack(&request, true, "").unwrap();
    assert!(ack.contains("<receiving_Document.creationDateTime>2026-10-26T12:13:14Z</receiving_Document.creationDateTime>"));
    valid(&ack);
    let matched = MatchingResult {
        quantities: vec![125; 24],
        statuses: vec!["12G".into(); 24],
    };
    let result = generate_result(&request, &matched, 1).unwrap();
    assert!(result.contains("<issuer_MarketParticipant.marketRole.roleCode>ZUK</issuer_MarketParticipant.marketRole.roleCode>"));
    valid(&result);
    let revised =
        parse_document(&NOMINT.replace("<version>1</version>", "<version>2</version>")).unwrap();
    let result2 = generate_result(&revised, &matched, 2).unwrap();
    assert_eq!(
        parse_document(&result).unwrap().id,
        parse_document(&result2).unwrap().id
    );
    let other = parse_document(&NOMINT.replace("REF-NOMINT", "ANOTHER-NOMINT")).unwrap();
    assert_ne!(
        parse_document(&result).unwrap().id,
        parse_document(&generate_result(&other, &matched, 1).unwrap())
            .unwrap()
            .id
    );
}
#[test]
fn gts_zso_nomres_role_variant_has_explicit_compatibility_evidence() {
    let x = AGGREGATE.replace(
        "<issuer_MarketParticipant.marketRole.roleCode>ZUK",
        "<issuer_MarketParticipant.marketRole.roleCode>ZSO",
    );
    let r = validate_xml(&x, &Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas"));
    assert!(r.valid, "{:?}", r.errors);
    assert!(r.checks.iter().any(|c| c.status == "pass"
        && c.detail.contains("ZSO")
        && c.detail.contains("compatibility")));
}
#[test]
fn programme_reasons_are_actual_codes_text_and_tristate_acceptance() {
    valid(PROCON);
    let b = value(PROCON);
    assert_eq!(b["accepted"], false);
    assert_eq!(
        b["programme_profiles"][0]["reason_codes"][0],
        serde_json::json!(["91G"])
    );
    assert_eq!(
        b["programme_profiles"][0]["reason_texts"][0],
        serde_json::json!(["Programme rejected by operator"])
    );
    assert!(b["reasons"].as_array().unwrap().iter().any(|r| r
        .as_str()
        .unwrap()
        .contains("91G: Programme rejected by operator")));
    assert_ne!(b["statuses"][0], "01G");
    assert_eq!(value(&PROCON.replace("91G", "82G"))["accepted"], true);
    assert_eq!(
        value(&PROCON.replace("91G", "21G"))["accepted"],
        serde_json::Value::Null
    );
    assert_eq!(value(&PROCON.replace("<Reason><reasonCode>91G</reasonCode><text>Programme rejected by operator</text></Reason>", ""))["accepted"], serde_json::Value::Null);
    let raw = value(&PROCON.replace("<quantity.amount>0", "<quantity.amount>88"));
    assert_eq!(raw["programme_profiles"][0]["quantities"][0], 88);
}
#[test]
fn acknowledgement_acceptance_with_warnings_preserves_actual_reason_codes() {
    let request = parse_document(NOMINT).unwrap();
    let ack = generate_ack(&request, true, "").unwrap();
    for code in ["93G", "95G"] {
        let xml = ack.replace(
            "<reasonCode>01G</reasonCode>",
            &format!("<reasonCode>{code}</reasonCode>"),
        );
        valid(&xml);
        let document = parse_document(&xml).unwrap();
        assert_eq!(document.accepted, Some(true), "{code}");
        assert!(document.reasons.iter().any(|reason| reason.contains(code)));
    }
    assert_eq!(reason_outcome(&["93G".into(), "95G".into()]), Some(true));
    assert_eq!(reason_outcome(&["93G".into(), "91G".into()]), Some(false));
    assert_eq!(reason_outcome(&["95G".into(), "21G".into()]), None);
}

#[test]
fn incomplete_programme_reasons_cannot_establish_acceptance() {
    let xml = PROCON.replace("91G", "82G");
    let reason =
        "<Reason><reasonCode>82G</reasonCode><text>Programme rejected by operator</text></Reason>";
    let incomplete = xml.replacen(reason, "", 1);
    assert_eq!(value(&incomplete)["accepted"], serde_json::Value::Null);
    assert!(
        !validate_xml(
            &incomplete,
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas")
        )
        .valid
    );
}
#[test]
fn per_profile_gaps_overlaps_and_unknown_points_remain_rejected() {
    let gap = NOMINT.replacen(
        "2026-10-28T05:00Z</timeInterval>",
        "2026-10-28T04:00Z</timeInterval>",
        1,
    );
    assert!(parse_document(&gap).is_err());
    let overlap = NOMINT.replacen("</Period>", "</Period><Period><timeInterval>2026-10-27T05:00Z/2026-10-28T05:00Z</timeInterval><direction.gasDirectionCode>Z02</direction.gasDirectionCode><quantity.amount>1</quantity.amount></Period>", 1);
    assert!(parse_document(&overlap).is_err());
    let r = validate_xml(
        &AGGREGATE.replace("GTSTRZ", "UNKNOWNPOINT"),
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas"),
    );
    assert!(!r.valid);
}

#[test]
fn counterpart_original_same_side_submission_is_preserved_on_mismatch() {
    let x = AGGREGATE.replace("<externalAccount codingScheme=\"ZSO\">END USER</externalAccount>", "<externalAccount codingScheme=\"ZSO\">GSLABCOUNTER</externalAccount>")
        .replace("GTSTRZ", "TTF")
        .replace("</External_Account>", "<InformationOrigin_TimeSeries><businessCode>18G</businessCode><Period><timeInterval>2026-10-27T05:00Z/2026-10-28T05:00Z</timeInterval><direction.gasDirectionCode>Z03</direction.gasDirectionCode><quantity.amount>299</quantity.amount></Period></InformationOrigin_TimeSeries></External_Account>");
    valid(&x);
    let b = value(&x);
    assert_eq!(
        b["nomination_profiles"][0]["counterparty_quantities"][0],
        299
    );
    assert_eq!(
        b["nomination_profiles"][0]["counterparty_directions"][0],
        "sell"
    );
}

#[test]
fn unknown_status_is_raw_parse_evidence_and_fails_pinned_codelist() {
    let x = NOMRES.replace("35G", "ZZZ");
    let b = value(&x);
    assert_eq!(
        b["nomination_profiles"][1]["status_codes"][0],
        serde_json::json!(["ZZZ", "37G"])
    );
    let r = validate_xml(&x, &Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas"));
    assert!(!r.valid);
    assert!(r
        .checks
        .iter()
        .any(|c| c.layer == "Pinned XSD" && c.status == "fail"));
}

#[test]
fn old_stored_business_json_defaults_new_fields_without_fabricating_timestamp() {
    let mut old = value(NOMINT);
    let map = old.as_object_mut().unwrap();
    for key in [
        "creation_date_time",
        "nomination_profiles",
        "programme_profiles",
    ] {
        map.remove(key);
    }
    let restored: BusinessDocument = serde_json::from_value(old).unwrap();
    let v = serde_json::to_value(&restored).unwrap();
    assert_eq!(v["creation_date_time"], "");
    assert_eq!(v["nomination_profiles"], serde_json::json!([]));
    assert_eq!(v["programme_profiles"], serde_json::json!([]));
    assert!(generate_ack(&restored, true, "").is_err());
}
