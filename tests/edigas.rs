use nomination_lab::edigas::*;
use std::path::Path;

fn draft(kind: &str) -> DraftInput {
    DraftInput {
        kind: kind.into(),
        gas_day: "2026-10-27".into(),
        counterparty: COUNTERPARTY_ACCOUNT.into(),
        direction: "buy".into(),
        quantities: vec![if kind == "PRODOC" { 0 } else { 100 }; 24],
        document_id: Some("LAB-REQUEST-1".into()),
        version: 2,
    }
}

#[test]
fn amsterdam_gas_days_cover_dst() {
    assert_eq!(intervals("2026-03-28").unwrap().len(), 23);
    assert_eq!(intervals("2026-10-24").unwrap().len(), 25);
    let normal = intervals("2026-10-27").unwrap();
    assert_eq!(normal.len(), 24);
    assert_eq!(normal[0].start, "2026-10-27T05:00:00Z");
    assert!(intervals("bad").is_err());
}

#[test]
fn matching_obeys_initial_and_previously_settled_rules() {
    let equal = match_nomination(&[100, 200], Some(&[100, 200]), None).unwrap();
    assert_eq!(equal.quantities, vec![100, 200]);
    assert_eq!(equal.statuses, vec!["12G", "12G"]);
    let mismatch = match_nomination(&[100, 200], Some(&[80, 220]), None).unwrap();
    assert_eq!(mismatch.quantities, vec![80, 200]);
    assert_eq!(mismatch.statuses, vec!["06G", "06G"]);
    let absent = match_nomination(&[100, 200], None, None).unwrap();
    assert_eq!(absent.quantities, vec![0, 0]);
    assert_eq!(absent.statuses, vec!["14G", "14G"]);
    let unchanged =
        match_nomination(&[100, 200], Some(&[80, 200]), Some(&[Some(75), Some(180)])).unwrap();
    assert_eq!(unchanged.quantities, vec![75, 200]);
    assert_eq!(unchanged.statuses, vec!["13G", "12G"]);
    assert!(match_nomination(&[1], Some(&[1, 2]), None).is_err());
}

#[test]
fn all_five_families_are_xsd_valid_and_references_round_trip() {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    for kind in ["NOMINT", "PRODOC"] {
        let input = draft(kind);
        let xml = generate_submission(&input).unwrap();
        let doc = parse_document(&xml).unwrap();
        assert_eq!(doc.kind, kind);
        assert_eq!(doc.quantities, input.quantities);
        assert_eq!(doc.version, 2);
        assert_eq!(doc.gas_day, input.gas_day);
        let report = validate_xml(&xml, &schema);
        assert!(report.valid, "{kind}: {:?}", report.errors);
        for accepted in [true, false] {
            let ack = generate_ack(&doc, accepted, "Local review").unwrap();
            let report = validate_xml(&ack, &schema);
            assert!(report.valid, "ACKNOW: {:?}", report.errors);
            let ack_doc = parse_document(&ack).unwrap();
            assert_eq!(ack_doc.reference.as_ref().unwrap().id, doc.id);
            assert_eq!(ack_doc.reference.as_ref().unwrap().version, 2);
            assert_eq!(ack_doc.accepted, Some(accepted));
        }
        let result = match_nomination(&doc.quantities, Some(&doc.quantities), None).unwrap();
        let xml = generate_result(&doc, &result, 7).unwrap();
        let report = validate_xml(&xml, &schema);
        assert!(report.valid, "result: {:?}", report.errors);
        let result_doc = parse_document(&xml).unwrap();
        assert_eq!(
            result_doc.kind,
            if kind == "NOMINT" { "NOMRES" } else { "PROCON" }
        );
        assert_eq!(result_doc.reference.unwrap().version, 2);
        assert_eq!(result_doc.version, 7);
        assert_eq!(result_doc.confirmed, doc.quantities);
    }
}

#[test]
fn hostile_xml_and_wrong_family_code_are_rejected() {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    assert!(
        parse_document("<!DOCTYPE x [<!ENTITY a SYSTEM 'file:///etc/passwd'>]><x>&a;</x>").is_err()
    );
    assert!(parse_document("<x>").is_err());
    let xml = generate_submission(&draft("NOMINT")).unwrap();
    assert!(
        !validate_xml(
            &xml.replace(
                "<documentCode>01G</documentCode>",
                "<documentCode>XXX</documentCode>"
            )
            .replace(
                "<documentCode>02G</documentCode>",
                "<documentCode>XXX</documentCode>"
            ),
            &schema
        )
        .valid
    );
    assert!(
        !validate_xml(
            &xml.replace("NominationDocument", "NominationResponseDocument"),
            &schema
        )
        .valid
    );
}

#[test]
fn compressed_periods_expand_and_profiles_cannot_have_gaps_or_overlaps() {
    let xml = generate_submission(&draft("NOMINT")).unwrap();
    let start = xml.find("<Period>").unwrap();
    let end = xml.rfind("</Period>").unwrap() + "</Period>".len();
    let period="<Period><timeInterval>2026-10-27T05:00Z/2026-10-28T05:00Z</timeInterval><direction.gasDirectionCode>Z02</direction.gasDirectionCode><quantity.amount>123</quantity.amount></Period>";
    let compact = format!("{}{}{}", &xml[..start], period, &xml[end..]);
    assert_eq!(parse_document(&compact).unwrap().quantities, vec![123; 24]);
    let gap = compact.replace(
        "2026-10-28T05:00Z</timeInterval>",
        "2026-10-28T04:00Z</timeInterval>",
    );
    assert!(parse_document(&gap).is_err());
    let overlap = format!("{}{}{}{}", &xml[..start], period, period, &xml[end..]);
    assert!(parse_document(&overlap).is_err());
}

#[test]
fn only_settled_hours_are_preserved_in_mixed_results() {
    let r = match_nomination(&[100, 200, 300], None, Some(&[Some(80), None, Some(0)])).unwrap();
    assert_eq!(r.quantities, vec![80, 0, 0]);
    assert_eq!(r.statuses, vec!["13G", "14G", "13G"]);
    assert!(match_nomination(&[-1], None, None).is_err());
}

#[test]
fn schema_profile_enforces_party_coding_and_schema_version() {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let xml = generate_submission(&draft("NOMINT")).unwrap();
    assert!(
        !validate_xml(
            &xml.replace("schemaVersion=\"1\"", "schemaVersion=\"2\""),
            &schema
        )
        .valid
    );
    assert!(
        !validate_xml(
            &xml.replace(
                "issuer_MarketParticipant.identification codingScheme=\"305\"",
                "issuer_MarketParticipant.identification codingScheme=\"ZSO\""
            ),
            &schema
        )
        .valid
    );
    assert!(
        !validate_xml(
            &xml.replace(
                "internalAccount codingScheme=\"ZSO\"",
                "internalAccount codingScheme=\"305\""
            ),
            &schema
        )
        .valid
    );
}

#[test]
fn schema_checksums_prevent_resource_substitution() {
    let dir = tempfile::tempdir().unwrap();
    let report = validate_xml(&generate_submission(&draft("NOMINT")).unwrap(), dir.path());
    assert!(!report.valid);
    assert!(report
        .errors
        .iter()
        .any(|e| e.contains("Missing pinned schema")));
}

#[test]
fn generated_dst_profiles_remain_xsd_valid() {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    for day in ["2026-03-28", "2026-10-24"] {
        for kind in ["NOMINT", "PRODOC"] {
            let mut d = draft(kind);
            d.gas_day = day.into();
            d.quantities =
                vec![if kind == "PRODOC" { 0 } else { 50 }; intervals(day).unwrap().len()];
            let xml = generate_submission(&d).unwrap();
            assert!(validate_xml(&xml, &schema).valid);
            assert_eq!(parse_document(&xml).unwrap().quantities, d.quantities);
        }
    }
}

#[test]
fn generators_reject_inputs_that_cannot_be_valid_business_xml() {
    let request = parse_document(&generate_submission(&draft("NOMINT")).unwrap()).unwrap();
    assert!(generate_ack(&request, false, "contains\0nul").is_err());
    assert!(generate_ack(&request, false, "").is_err());
    let mut invalid = draft("PRODOC");
    invalid.quantities[0] = 1;
    assert!(generate_submission(&invalid).is_err());
    let mut invalid = draft("NOMINT");
    invalid.quantities.pop();
    assert!(generate_submission(&invalid).is_err());
    assert!(parse_document(&"x".repeat(1_000_001)).is_err());
}
