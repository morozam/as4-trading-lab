use crate::app::{id, now, process_get, process_save, App, Result};
use rusqlite::{params, OptionalExtension};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

#[derive(Debug)]
struct Job {
    id: i64,
    message: String,
    process: String,
    xml: String,
    ref_to: Option<String>,
    attempts: i64,
}

pub async fn run(app: Arc<App>) {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("HTTP client");
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    let mut tick = 0;
    loop {
        ticker.tick().await;
        tick += 1;
        if tick % 10 == 1 {
            let url = reqwest::Url::parse(&app.config.gateway_url);
            let reachable = if let Ok(url) = url {
                tokio::time::timeout(
                    Duration::from_millis(350),
                    tokio::net::TcpStream::connect((
                        url.host_str().unwrap_or("127.0.0.1"),
                        url.port_or_known_default().unwrap_or(80),
                    )),
                )
                .await
                .is_ok_and(|r| r.is_ok())
            } else {
                false
            };
            app.gateway_reachable.store(reachable, Ordering::Relaxed);
        }
        if let Err(e) = dispatch_one(&app, &client).await {
            eprintln!("Outbox worker: {e}");
        }
    }
}

pub async fn dispatch_one(app: &Arc<App>, client: &reqwest::Client) -> Result<bool> {
    let job = {
        let mut db = app.db.lock().unwrap();
        let tx = db.transaction()?;
        let job:Option<Job>=tx.query_row("SELECT id,message_id,process_id,payload,ref_to,attempts FROM outbox WHERE status='queued' AND available_at<=? ORDER BY id LIMIT 1",[chrono::Utc::now().timestamp()],|r|Ok(Job{id:r.get(0)?,message:r.get(1)?,process:r.get(2)?,xml:r.get(3)?,ref_to:r.get(4)?,attempts:r.get(5)?})).optional()?;
        if let Some(j) = &job {
            tx.execute("UPDATE outbox SET status='dispatching',attempts=attempts+1,started_at=? WHERE id=?",params![now(),j.id])?;
            let mut p = process_get(&tx, &j.process)?;
            p.handoff_status = "Dispatching".into();
            process_save(&tx, &p)?;
        }
        tx.commit()?;
        job
    };
    let Some(job) = job else { return Ok(false) };
    let mut req = client
        .post(&app.config.gateway_url)
        .header("content-type", "application/xml")
        .header("x-holodeckb2b-pmodeid", &app.config.pmode_id)
        .header("x-holodeckb2b-messageid", &job.message)
        .header("x-holodeckb2b-conversationid", &job.process)
        .header("x-holodeckb2b-timestamp", now())
        .header("x-holodeckb2b-containment", "ATTACHMENT")
        .body(job.xml.clone());
    if let Some(reference) = &job.ref_to {
        req = req.header("x-holodeckb2b-reftomessageid", reference)
    }
    let result = req.send().await;
    let (status, detail, retry) = match result {
        Ok(response) => {
            let code = response.status().as_u16();
            let error = response
                .headers()
                .get("x-error")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if code == 202 {
                (
                    "accepted",
                    format!("Holodeck accepted the handoff (HTTP {code})"),
                    false,
                )
            } else if code == 400 {
                (
                    "rejected",
                    format!("Holodeck rejected submission: {error}"),
                    false,
                )
            } else {
                (
                    "unknown",
                    format!("Unexpected HTTP {code}; acceptance is uncertain. {error}"),
                    false,
                )
            }
        }
        Err(error) if error.is_connect() => {
            if job.attempts < 2 {
                (
                    "queued",
                    format!("Connection not established; safe retry scheduled: {error}"),
                    true,
                )
            } else {
                (
                    "rejected",
                    format!("Gateway unavailable after three pre-connect attempts: {error}"),
                    false,
                )
            }
        }
        Err(error) => (
            "unknown",
            format!(
                "Handoff response lost or timed out; investigate {}: {error}",
                job.message
            ),
            false,
        ),
    };
    record_handoff(app, &job.message, status, &detail, retry)?;
    Ok(true)
}

pub fn record_handoff(
    app: &Arc<App>,
    message: &str,
    status: &str,
    detail: &str,
    retry: bool,
) -> Result<()> {
    let job = {
        let db = app.db.lock().unwrap();
        db.query_row("SELECT id,message_id,process_id,payload,ref_to,attempts FROM outbox WHERE message_id=?", [message], |r| Ok(Job{id:r.get(0)?,message:r.get(1)?,process:r.get(2)?,xml:r.get(3)?,ref_to:r.get(4)?,attempts:r.get(5)?}))?
    };
    let detail = detail.to_string();
    let mut db = app.db.lock().unwrap();
    let tx = db.transaction()?;
    // A receipt callback can arrive before submit returns. Preserve positive evidence.
    let mut p = process_get(&tx, &job.process)?;
    let has_receipt: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM callbacks WHERE kind='receipt' AND json_extract(headers, '$.\"x-holodeckb2b-reftomessageid\"')=?)",[&job.message],|r|r.get(0))?;
    let effective_status = if status == "unknown" && has_receipt {
        "accepted"
    } else {
        status
    };
    tx.execute(
        "UPDATE outbox SET status=?,error=?,available_at=? WHERE id=?",
        params![
            effective_status,
            if effective_status == "accepted" {
                None
            } else {
                Some(detail.clone())
            },
            chrono::Utc::now().timestamp() + 2_i64.pow(job.attempts.min(5) as u32),
            job.id
        ],
    )?;
    let handoff_label: String = match effective_status {
        "accepted" => "Gateway accepted",
        "queued" => "Queued",
        "rejected" => "Rejected before acceptance",
        _ => "Delivery unknown",
    }
    .into();
    let latest: String = tx.query_row(
        "SELECT message_id FROM outbox WHERE process_id=? ORDER BY id DESC LIMIT 1",
        [&job.process],
        |r| r.get(0),
    )?;
    if latest == job.message {
        p.handoff_status = handoff_label.clone();
    }
    let is_request = p.source_message_id == job.message && app.config.role == "brp";
    if is_request && ["Queued", "Delivery unknown"].contains(&p.stage.as_str()) {
        p.stage = match effective_status {
            "accepted" => "Awaiting ACKNOW",
            "queued" => "Queued",
            "rejected" => "Submission failed",
            _ => "Delivery unknown",
        }
        .into();
    }
    p.events.push(crate::app::Event {
        id: id(),
        kind: "handoff".into(),
        title: if retry {
            "Gateway retry scheduled"
        } else {
            &handoff_label
        }
        .into(),
        detail: detail.clone(),
        at: now(),
        message_id: Some(job.message.clone()),
    });
    process_save(&tx, &p)?;
    let raw: String = tx.query_row(
        "SELECT data FROM messages WHERE id=?",
        [&job.message],
        |r| r.get(0),
    )?;
    let mut m: crate::app::Message = serde_json::from_str(&raw)?;
    m.transport_status = if has_receipt {
        "Receipt received".into()
    } else {
        handoff_label
    };
    tx.execute(
        "UPDATE messages SET data=? WHERE id=?",
        params![serde_json::to_string(&m)?, job.message],
    )?;
    tx.commit()?;
    Ok(())
}
