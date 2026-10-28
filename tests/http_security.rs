use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use nomination_lab::{
    app::{App, Config},
    web,
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn local_role_sessions_and_csrf_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(Config::local("brp", dir.path().into())).unwrap();
    let router = web::router(app.clone());
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let wrong = json!({"username":"tso","password":"local-tso"});
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(wrong.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let login = json!({"username":"brp","password":"local-brp"});
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(login.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert!(cookie.starts_with("lab_brp_session="));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let session: Value = serde_json::from_slice(&bytes).unwrap();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/validate")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from("{\"xml\":\"<bad/>\"}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/validate")
                .header("host", "127.0.0.1:8787")
                .header("origin", "https://unexpected.example")
                .header("cookie", &cookie)
                .header("x-csrf-token", session["csrf"].as_str().unwrap())
                .header("content-type", "application/json")
                .body(Body::from("{\"xml\":\"<bad/>\"}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/fixtures/any")
                .header("cookie", &cookie)
                .header("x-csrf-token", session["csrf"].as_str().unwrap())
                .header("content-type", "application/json")
                .body(Body::from("{\"quantities\":null}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/integrations/wrong/deliver")
                .body(Body::from("<xml/>"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
