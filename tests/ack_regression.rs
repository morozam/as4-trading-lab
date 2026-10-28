use nomination_lab::edigas::*;

#[test]
fn gts_positive_acknowledgement_variants_are_not_rejections() {
    let request = DraftInput {
        kind: "NOMINT".into(),
        gas_day: "2026-11-03".into(),
        counterparty: COUNTERPARTY_ACCOUNT.into(),
        direction: "sell".into(),
        quantities: vec![10000; 24],
        document_id: Some("NOM-POSITIVE-CODES".into()),
        version: 1,
    };
    let parsed = parse_document(&generate_submission(&request).unwrap()).unwrap();
    let xml = generate_ack(&parsed, true, "").unwrap();
    for code in ["01G", "01H", "84G", "92G", "02H"] {
        let variant = xml.replace(
            "<reasonCode>01G</reasonCode>",
            &format!("<reasonCode>{code}</reasonCode>"),
        );
        assert_eq!(
            parse_document(&variant).unwrap().accepted,
            Some(true),
            "{code}"
        );
    }
}
