use crate::{
    app::{App, Config, Error, Result},
    edigas,
};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tower_http::services::{ServeDir, ServeFile};

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({"error":self.message})),
        )
            .into_response()
    }
}
fn cookie_name(app: &App) -> String {
    format!("lab_{}_session", app.config.role)
}
fn cookie_token(app: &App, headers: &HeaderMap) -> String {
    let name = format!("{}=", cookie_name(app));
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .find_map(|s| s.trim().strip_prefix(&name))
        .unwrap_or("")
        .to_string()
}
fn check_origin(headers: &HeaderMap) -> Result<()> {
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if origin != format!("http://{host}") && origin != format!("https://{host}") {
            return Err(Error {
                status: 403,
                message: "Cross-origin commands are not allowed".into(),
            });
        }
    }
    Ok(())
}
fn auth(app: &App, headers: &HeaderMap, mutation: bool) -> Result<String> {
    let token = cookie_token(app, headers);
    let csrf = app.session(&token)?;
    if mutation {
        check_origin(headers)?;
        if headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) != Some(&csrf) {
            return Err(Error {
                status: 403,
                message: "Invalid session command token; sign in again".into(),
            });
        }
    }
    Ok(csrf)
}
fn session_json(app: &App, csrf: String) -> Value {
    json!({"role":app.config.role,"user":app.config.username,"csrf":csrf})
}

pub fn router(app: Arc<App>) -> Router {
    let ui = app.config.ui_dir.clone();
    Router::new()
        .route("/api/info", get(info))
        .route("/api/login", post(login))
        .route("/api/session", get(session))
        .route("/api/logout", post(logout))
        .route("/api/calendar", get(calendar))
        .route("/api/state", get(state))
        .route("/api/gateway/inventory", get(inventory))
        .route("/api/balancing/preview", post(balancing_preview))
        .route("/api/balancing/send", post(balancing_send))
        .route("/api/programmes/default", post(default_programme))
        .route("/api/messages/{id}/details", get(message_details))
        .route("/api/drafts", post(create_draft))
        .route("/api/drafts/{id}", put(update_draft))
        .route("/api/drafts/{id}/preview", post(preview))
        .route("/api/drafts/{id}/send", post(send))
        .route("/api/processes/{id}/revise", post(revise))
        .route("/api/processes/{id}/ack", post(ack))
        .route("/api/processes/{id}/result", post(result))
        .route("/api/fixtures/{id}", put(fixture))
        .route("/api/validate", post(validate))
        .route("/api/messages/{id}/xml", get(xml))
        .route("/integrations/{token}/deliver", post(deliver))
        .route("/integrations/{token}/notify/{kind}", post(signal))
        .route(
            "/api/{*path}",
            get(api_not_found).post(api_not_found).put(api_not_found),
        )
        .fallback_service(
            ServeDir::new(&ui).not_found_service(ServeFile::new(ui.join("index.html"))),
        )
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(app)
}
async fn info(State(a): State<Arc<App>>) -> Json<Value> {
    Json(json!({"role":a.config.role,"environment":"LOCAL AS4","peer_url":a.config.peer_url}))
}
#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}
async fn login(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(v): Json<Login>,
) -> Result<Response> {
    check_origin(&headers)?;
    let (token, csrf) = a.login(&v.username, &v.password)?;
    let mut response = Json(session_json(&a, csrf)).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200",
            cookie_name(&a)
        ))
        .unwrap(),
    );
    Ok(response)
}
async fn session(State(a): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    Ok(Json(session_json(&a, auth(&a, &headers, false)?)))
}
async fn logout(State(a): State<Arc<App>>, headers: HeaderMap) -> Result<Response> {
    auth(&a, &headers, true)?;
    a.logout(&cookie_token(&a, &headers))?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
            cookie_name(&a)
        ))
        .unwrap(),
    );
    Ok(response)
}
async fn state(State(a): State<Arc<App>>, headers: HeaderMap) -> Result<Json<crate::app::State>> {
    auth(&a, &headers, false)?;
    Ok(Json(a.state()?))
}
async fn calendar(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    auth(&a, &headers, false)?;
    let day = q.get("day").ok_or_else(|| Error::bad("Choose a gas day"))?;
    Ok(Json(json!({"hours":edigas::intervals(day)?})))
}
async fn create_draft(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(v): Json<edigas::DraftInput>,
) -> Result<Json<crate::app::Draft>> {
    auth(&a, &headers, true)?;
    Ok(Json(a.create_draft(v)?))
}
async fn update_draft(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(v): Json<edigas::DraftInput>,
) -> Result<Json<crate::app::Draft>> {
    auth(&a, &headers, true)?;
    Ok(Json(a.update_draft(&id, v)?))
}
async fn preview(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::app::Preview>> {
    auth(&a, &headers, true)?;
    Ok(Json(a.preview(&id)?))
}
#[derive(Deserialize)]
struct Send {
    hash: String,
    idempotency_key: String,
}
async fn send(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(v): Json<Send>,
) -> Result<Json<Value>> {
    auth(&a, &headers, true)?;
    Ok(Json(
        json!({"process_id":a.send_draft(&id,&v.hash,&v.idempotency_key)?}),
    ))
}
async fn revise(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::app::Draft>> {
    auth(&a, &headers, true)?;
    Ok(Json(a.revise(&id)?))
}
#[derive(Deserialize)]
struct Ack {
    accepted: bool,
    reason: String,
    idempotency_key: String,
}
async fn ack(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(v): Json<Ack>,
) -> Result<Json<Value>> {
    auth(&a, &headers, true)?;
    Ok(Json(
        json!({"message_id":a.acknowledge(&id,v.accepted,&v.reason,&v.idempotency_key)?}),
    ))
}
#[derive(Deserialize)]
struct Key {
    idempotency_key: String,
}
async fn result(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(v): Json<Key>,
) -> Result<Json<Value>> {
    auth(&a, &headers, true)?;
    let (mid, result) = a.send_result(&id, &v.idempotency_key)?;
    Ok(Json(json!({"message_id":mid,"result":result})))
}
#[derive(Deserialize)]
struct Fixture {
    quantities: Option<Vec<i64>>,
}
async fn fixture(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(v): Json<Fixture>,
) -> Result<Json<Value>> {
    auth(&a, &headers, true)?;
    a.set_fixture(&id, v.quantities)?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
struct Xml {
    xml: String,
}
async fn validate(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(v): Json<Xml>,
) -> Result<Json<edigas::ValidationReport>> {
    auth(&a, &headers, true)?;
    Ok(Json(edigas::validate_xml(&v.xml, &a.config.schema_dir)))
}
async fn xml(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response> {
    auth(&a, &headers, false)?;
    let xml = a.xml(&id)?;
    let mut response = xml.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if q.get("download").is_some_and(|v| v == "1") {
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"edigas.xml\""),
        );
    }
    Ok(response)
}
fn callback_headers(a: &App, token: &str, headers: &HeaderMap) -> Result<Value> {
    if token != a.config.callback_token {
        return Err(Error {
            status: 404,
            message: "Unknown integration route".into(),
        });
    }
    let mut values = serde_json::Map::new();
    for (key, value) in headers {
        if key.as_str().starts_with("x-holodeckb2b-") || key.as_str() == "content-type" {
            values.insert(key.as_str().into(), json!(value.to_str().unwrap_or("")));
        }
    }
    // The configured receiver PMode is the trusted local integration boundary.
    if let Some(pmode) = values.get("x-holodeckb2b-pmodeid").and_then(Value::as_str) {
        if pmode != a.config.inbound_pmode_id && pmode != a.config.pmode_id {
            return Err(Error {
                status: 403,
                message: "Unexpected gateway route".into(),
            });
        }
    }
    Ok(Value::Object(values))
}
async fn deliver(
    State(a): State<Arc<App>>,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<StatusCode> {
    let metadata = callback_headers(&a, &token, &headers)?;
    let message = metadata
        .get("x-holodeckb2b-messageid")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::bad("Gateway omitted MessageId"))?
        .to_string();
    a.receive_business(&message, metadata, &body)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn signal(
    State(a): State<Arc<App>>,
    Path((token, kind)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode> {
    if !["receipt", "error"].contains(&kind.as_str()) {
        return Err(Error {
            status: 404,
            message: "Unknown signal".into(),
        });
    }
    let metadata = callback_headers(&a, &token, &headers)?;
    a.receive_signal(&kind, metadata)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn api_not_found() -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error":"Unknown API operation"})),
    )
}

pub async fn serve(config: Config) -> Result<()> {
    let bind = format!("{}:{}", config.bind, config.port);
    let app = App::open(config)?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    eprintln!(
        "{} node listening on http://{}",
        app.config.role.to_uppercase(),
        bind
    );
    tokio::spawn(crate::transport::run(app.clone()));
    axum::serve(listener, router(app))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn inventory(State(a): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    auth(&a, &headers, false)?;
    let cfg = a.config.clone();
    let value = tokio::task::spawn_blocking(move || crate::inventory::read_inventory(&cfg))
        .await
        .map_err(|_| Error::bad("Gateway inventory could not be read"))??;
    Ok(Json(value))
}
async fn balancing_preview(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(v): Json<edigas::BalancingInput>,
) -> Result<Json<crate::app::BalancingPreview>> {
    auth(&a, &headers, true)?;
    Ok(Json(a.preview_balancing(v)?))
}
#[derive(Deserialize)]
struct BalancingSend {
    preview_id: String,
    hash: String,
    idempotency_key: String,
}
async fn balancing_send(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(v): Json<BalancingSend>,
) -> Result<Json<Value>> {
    auth(&a, &headers, true)?;
    Ok(Json(
        json!({"process_id":a.send_balancing(&v.preview_id,&v.hash,&v.idempotency_key)?}),
    ))
}
#[derive(Deserialize)]
struct DefaultProgramme {
    gas_day: String,
    idempotency_key: String,
}
async fn default_programme(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(v): Json<DefaultProgramme>,
) -> Result<Json<Value>> {
    auth(&a, &headers, true)?;
    Ok(Json(
        json!({"process_id":a.send_default_programme(&v.gas_day,&v.idempotency_key)?}),
    ))
}
async fn message_details(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    auth(&a, &headers, false)?;
    Ok(Json(a.message_details(&id)?))
}
