use nomination_lab::app::{App, Config};
use nomination_lab::edigas::{self, DraftInput};
use std::path::PathBuf;

fn node(role: &str) -> (tempfile::TempDir, std::sync::Arc<App>) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::local(role, dir.path().to_path_buf());
    cfg.schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let app = App::open(cfg).unwrap();
    (dir, app)
}
fn input() -> DraftInput {
    DraftInput {
        kind: "NOMINT".into(),
        gas_day: "2026-11-03".into(),
        counterparty: edigas::COUNTERPARTY_ACCOUNT.into(),
        direction: "sell".into(),
        quantities: vec![10_000; 24],
        document_id: None,
        version: 1,
    }
}

#[test]
fn sending_requires_current_review_and_is_idempotent() {
    let (_dir, app) = node("brp");
    let draft = app.create_draft(input()).unwrap();
    assert!(app
        .send_draft(&draft.id, "not-reviewed", "command-1")
        .is_err());
    let review = app.preview(&draft.id).unwrap();
    assert!(review.validation.valid, "{:?}", review.validation.errors);
    let first = app
        .send_draft(&draft.id, &review.hash, "command-1")
        .unwrap();
    let second = app
        .send_draft(&draft.id, &review.hash, "command-1")
        .unwrap();
    assert_eq!(first, second);
    let state = app.state().unwrap();
    assert_eq!(state.processes.len(), 1);
    assert_eq!(state.connection.queued, 1);
    assert_eq!(state.processes[0].stage, "Queued");
}

#[test]
fn editing_invalidates_reviewed_xml() {
    let (_dir, app) = node("brp");
    let draft = app.create_draft(input()).unwrap();
    let review = app.preview(&draft.id).unwrap();
    let mut next = input();
    next.quantities = vec![9000; 24];
    app.update_draft(&draft.id, next).unwrap();
    assert!(app
        .send_draft(&draft.id, &review.hash, "stale-command")
        .is_err());
}

#[test]
fn duplicate_delivery_does_not_create_a_second_process_and_conflicts_fail() {
    let (_dir, app) = node("tso");
    let mut request = input();
    request.document_id = Some("NOM-LAB-ONE".into());
    let xml = edigas::generate_submission(&request).unwrap();
    app.receive_business("message-1@test", serde_json::json!({}), &xml)
        .unwrap();
    app.receive_business("message-1@test", serde_json::json!({}), &xml)
        .unwrap();
    app.receive_business("message-2@test", serde_json::json!({}), &xml)
        .unwrap();
    assert_eq!(app.state().unwrap().processes.len(), 1);
    assert!(app
        .receive_business(
            "message-1@test",
            serde_json::json!({}),
            &xml.replace("10000", "9000")
        )
        .is_err());
}

#[test]
fn independently_delivered_ack_and_result_change_brp_state() {
    let (_a, brp) = node("brp");
    let (_b, tso) = node("tso");
    let draft = brp.create_draft(input()).unwrap();
    let review = brp.preview(&draft.id).unwrap();
    let process_id = brp.send_draft(&draft.id, &review.hash, "send-1").unwrap();
    tso.receive_business("request@test", serde_json::json!({}), &review.xml)
        .unwrap();
    let inbound = tso.state().unwrap().processes[0].id.clone();
    tso.acknowledge(&inbound, true, "", "ack-1").unwrap();
    let ack = tso
        .state()
        .unwrap()
        .messages
        .into_iter()
        .find(|m| m.family == "ACKNOW")
        .unwrap();
    brp.receive_business("ack@test", serde_json::json!({}), &ack.xml)
        .unwrap();
    let state = brp.state().unwrap();
    assert_eq!(state.processes[0].ack_status, "Accepted");
    assert!(state.processes[0].confirmed.is_empty());
    tso.set_fixture(&inbound, Some(vec![8000; 24])).unwrap();
    tso.send_result(&inbound, "result-1").unwrap();
    let result = tso
        .state()
        .unwrap()
        .messages
        .into_iter()
        .find(|m| m.family == "NOMRES")
        .unwrap();
    brp.receive_business("result@test", serde_json::json!({}), &result.xml)
        .unwrap();
    let state = brp.state().unwrap();
    let process = state.processes.iter().find(|p| p.id == process_id).unwrap();
    assert_eq!(process.stage, "Mismatch");
    assert_eq!(process.confirmed, vec![8000; 24]);
    assert_eq!(process.quantities, vec![10000; 24]);
}

#[test]
fn role_is_enforced_and_history_survives_reopen() {
    let (dir, app) = node("brp");
    let draft = app.create_draft(input()).unwrap();
    assert!(app.set_fixture("anything", None).is_err());
    let config = app.config.clone();
    drop(app);
    let reopened = App::open(config).unwrap();
    assert_eq!(reopened.state().unwrap().drafts[0].id, draft.id);
    assert!(dir.path().join("app.sqlite3").exists());
}

#[test]
fn fixture_cannot_be_applied_before_business_acceptance() {
    let (_dir, tso) = node("tso");
    let mut request = input();
    request.document_id = Some("NOM-ACK-BEFORE-FIXTURE".into());
    let xml = edigas::generate_submission(&request).unwrap();
    tso.receive_business("fixture-request@test", serde_json::json!({}), &xml)
        .unwrap();
    let inbound = tso.state().unwrap().processes[0].id.clone();
    assert!(tso.set_fixture(&inbound, Some(vec![10000; 24])).is_err());
}

#[test]
fn result_scope_and_acknowledgement_family_must_match_the_request() {
    for variant in ["day", "direction", "ack-family"] {
        let (_dir, brp) = node("brp");
        let draft = brp.create_draft(input()).unwrap();
        let review = brp.preview(&draft.id).unwrap();
        brp.send_draft(&draft.id, &review.hash, "scope-send")
            .unwrap();
        let mut req = edigas::parse_document(&review.xml).unwrap();
        let xml = if variant == "ack-family" {
            edigas::generate_ack(&req, true, "").unwrap().replace(
                "<receiving_Document.documentCode>02G</receiving_Document.documentCode>",
                "<receiving_Document.documentCode>ALH</receiving_Document.documentCode>",
            )
        } else {
            if variant == "day" {
                req.gas_day = "2026-11-04".into()
            } else {
                req.direction = "buy".into()
            }
            edigas::generate_result(
                &req,
                &edigas::MatchingResult {
                    quantities: vec![10000; 24],
                    statuses: vec!["12G".into(); 24],
                },
                1,
            )
            .unwrap()
        };
        brp.receive_business("wrong-scope@test", serde_json::json!({}), &xml)
            .unwrap();
        let state = brp.state().unwrap();
        assert!(
            state.processes[0].confirmed.is_empty(),
            "{variant} result must not be applied"
        );
        assert_eq!(
            state.processes[0].ack_status, "Awaiting",
            "{variant} ACK must not be applied"
        );
        assert!(
            state
                .messages
                .iter()
                .any(|m| m.transport_status.contains("scope")),
            "{variant} must remain inspectable as a scope exception"
        );
    }
}

#[test]
fn revision_cannot_reverse_or_change_the_counterparty_of_a_settled_stream() {
    let (_dir, brp) = node("brp");
    let draft = brp.create_draft(input()).unwrap();
    let review = brp.preview(&draft.id).unwrap();
    let pid = brp
        .send_draft(&draft.id, &review.hash, "revision-scope")
        .unwrap();
    let revision = brp.revise(&pid).unwrap();
    let mut changed = revision.input.clone();
    changed.direction = "buy".into();
    assert!(brp.update_draft(&revision.id, changed).is_err());
    let mut changed = revision.input;
    changed.counterparty = "OTHER".into();
    assert!(brp.update_draft(&revision.id, changed).is_err());
}

#[test]
fn receipt_for_an_earlier_reply_does_not_resolve_a_later_handoff() {
    let (_dir, tso) = node("tso");
    let mut req = input();
    req.document_id = Some("NOM-SIGNAL-SCOPE".into());
    tso.receive_business(
        "signal-request@test",
        serde_json::json!({}),
        &edigas::generate_submission(&req).unwrap(),
    )
    .unwrap();
    let pid = tso.state().unwrap().processes[0].id.clone();
    let ack = tso.acknowledge(&pid, true, "", "signal-ack").unwrap();
    tso.receive_signal("receipt",serde_json::json!({"x-holodeckb2b-messageid":"ack-receipt@test","x-holodeckb2b-reftomessageid":ack})).unwrap();
    tso.set_fixture(&pid, Some(vec![10000; 24])).unwrap();
    let (result, _) = tso.send_result(&pid, "signal-result").unwrap();
    nomination_lab::transport::record_handoff(
        &tso,
        &result,
        "unknown",
        "Simulated timeout after possible write",
        false,
    )
    .unwrap();
    let db = tso.db.lock().unwrap();
    let status: String = db
        .query_row(
            "SELECT status FROM outbox WHERE message_id=?",
            [&result],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "unknown");
    drop(db);
    tso.receive_signal("receipt",serde_json::json!({"x-holodeckb2b-messageid":"result-receipt@test","x-holodeckb2b-reftomessageid":result})).unwrap();
    let db = tso.db.lock().unwrap();
    let status: String = db
        .query_row(
            "SELECT status FROM outbox WHERE message_id=?",
            [&result],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "accepted");
}

#[test]
fn earlier_handoff_completion_preserves_the_latest_reply_and_receipt_evidence() {
    let (_dir, tso) = node("tso");
    let mut req = input();
    req.document_id = Some("NOM-HANDOFF-ORDER".into());
    tso.receive_business(
        "order-request@test",
        serde_json::json!({}),
        &edigas::generate_submission(&req).unwrap(),
    )
    .unwrap();
    let pid = tso.state().unwrap().processes[0].id.clone();
    let ack = tso.acknowledge(&pid, true, "", "order-ack").unwrap();
    tso.receive_signal("receipt",serde_json::json!({"x-holodeckb2b-messageid":"order-receipt@test","x-holodeckb2b-reftomessageid":ack})).unwrap();
    tso.set_fixture(&pid, Some(vec![10000; 24])).unwrap();
    tso.send_result(&pid, "order-result").unwrap();
    nomination_lab::transport::record_handoff(
        &tso,
        &ack,
        "accepted",
        "Late HTTP202 for earlier ACKNOW",
        false,
    )
    .unwrap();
    let state = tso.state().unwrap();
    assert_eq!(
        state.processes[0].handoff_status, "Queued",
        "Later result is still queued"
    );
    assert_eq!(
        state
            .messages
            .iter()
            .find(|m| m.id == ack)
            .unwrap()
            .transport_status,
        "Receipt received"
    );
}

#[tokio::test]
async fn interrupted_handoff_remains_unknown_after_restart_and_is_not_resent() {
    let (dir, app) = node("brp");
    let draft = app.create_draft(input()).unwrap();
    let preview = app.preview(&draft.id).unwrap();
    let pid = app
        .send_draft(&draft.id, &preview.hash, "crash-send")
        .unwrap();
    app.db
        .lock()
        .unwrap()
        .execute("UPDATE outbox SET status='dispatching'", [])
        .unwrap();
    let mid = app.state().unwrap().messages[0].id.clone();
    let config = app.config.clone();
    drop(app);
    let reopened = App::open(config).unwrap();
    let state = reopened.state().unwrap();
    assert_eq!(state.processes[0].id, pid);
    assert_eq!(state.processes[0].handoff_status, "Delivery unknown");
    assert_eq!(state.connection.unknown, 1);
    assert_eq!(state.connection.queued, 0);
    assert_eq!(state.messages[0].xml, preview.xml);
    assert_eq!(state.messages[0].hash, preview.hash);
    assert_eq!(state.messages[0].id, mid);
    assert!(
        !nomination_lab::transport::dispatch_one(&reopened, &reqwest::Client::new())
            .await
            .unwrap()
    );
    reopened.receive_signal("receipt",serde_json::json!({"x-holodeckb2b-messageid":"crash-receipt@test","x-holodeckb2b-reftomessageid":mid})).unwrap();
    assert_eq!(reopened.state().unwrap().connection.unknown, 0);
    assert!(dir.path().join("app.sqlite3").exists());
}
