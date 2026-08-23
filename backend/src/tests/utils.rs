// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use std::{fmt::Display, rc::Rc};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde::Deserialize;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::*;

#[derive(Deserialize)]
pub(super) struct URLData {
    #[serde(default, alias = "shorturl")]
    pub(super) shortlink: String,
    #[serde(default, alias = "longurl")]
    pub(super) longlink: String,
    #[serde(default)]
    pub(super) hits: i64,
    #[serde(default)]
    pub(super) expiry_time: i64,
    #[serde(default)]
    pub(super) notes: String,
    #[serde(default)]
    pub(super) reason: String,
}

#[derive(Deserialize)]
pub(super) struct BackendConfig {
    pub(super) version: String,
    pub(super) slug_length: usize,
}

pub(super) async fn body_string(resp: Response) -> String {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub(super) fn default_config(test: &str) -> config::Config {
    config::Config {
        listen_address: String::from("0.0.0.0"),
        port: 4567,
        db_location: format!("/tmp/chhoto-url-test-{test}.sqlite"),
        cache_control_header: None,
        disable_frontend: true,
        site_url: Some(String::from("https://mydomain.com")),
        public_mode: false,
        public_mode_expiry_delay: None,
        use_temp_redirect: false,
        allowed_protocols: Vec::from(["http", "https", "ftp", "magnet"].map(|s| s.to_string())),
        password: Some(String::from("testpass")),
        hash_algorithm: config::HashAlgorithm::None,
        api_key: Some(String::from(
            "Z8FNjh2J2v3yfb0xPDIVA58Pj4D0e2jSERVdoqM5pJCbU2w5tmg3PNioD6GUhaQwHHaDLBNZj0EQE8MS4TLKcUyusa05",
        )),
        slug_style: config::SlugStyle::Pair,
        slug_length: 8,
        try_longer_slug: false,
        allow_capital_letters: false,
        custom_landing_directory: None,
        use_wal_mode: true,
        ensure_acid: false,
        frontend_page_size: 10,
    }
}

pub(super) async fn create_app(conf: &config::Config, test: &str) -> (TempDir, Router) {
    let tempdir = TempDir::new().unwrap();
    let db_file = tempdir.path().join(format!("{test}.sqlite"));
    let writer = Arc::from(Mutex::from(database::open_db(
        db_file.to_str().unwrap(),
        false,
    )));
    database::init_db(
        &mut *writer.lock().await,
        conf.use_wal_mode,
        conf.ensure_acid,
    );

    let (hits_tx, hits_rx) = mpsc::channel::<(String, bool)>(1024);
    background::spawn_hits_worker(Arc::clone(&writer), hits_rx);

    let state = Arc::new(AppState {
        hits_tx,
        reader: Mutex::new(database::open_db(db_file.to_str().unwrap(), false)),
        writer,
        config: conf.clone(),
    });

    (tempdir, services::build_router(state, conf))
}

pub(super) async fn add_link<S: Display>(
    app: &Router,
    api_key: &str,
    shortlink: S,
    expiry_delay: i64,
    notes: &str,
) -> (StatusCode, URLData) {
    let req = Request::post("/api/new")
        .header("X-API-Key", api_key)
        .body(Body::from(format!(
            "{{\"shortlink\":\"{shortlink}\",\"longlink\":\"https://example-{shortlink}.com\",\"expiry_delay\":{expiry_delay},\"notes\":\"{notes}\"}}"
        )))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let url: URLData = serde_json::from_str(&body_string(resp).await).unwrap();

    (status, url)
}

pub(super) async fn expand<S: Display>(
    app: &Router,
    api_key: &str,
    shortlink: S,
) -> (StatusCode, URLData) {
    let req = Request::post("/api/expand")
        .header("X-API-Key", api_key)
        .body(Body::from(shortlink.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let url: URLData = serde_json::from_str(&body_string(resp).await).unwrap();

    (status, url)
}

pub(super) async fn getall(app: &Router, api_key: &str, params: &str) -> Rc<[URLData]> {
    let req = Request::get(format!("/api/all?{params}"))
        .header("X-API-Key", api_key)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert!(resp.status().is_success());
    let reply_chunks: Rc<[URLData]> = serde_json::from_str(&body_string(resp).await).unwrap();

    reply_chunks
}

pub(super) async fn edit_link(
    app: &Router,
    api_key: &str,
    shortlink: &str,
    reset_hits: bool,
    expiry_time: Option<i64>,
    notes: Option<&str>,
) -> StatusCode {
    let mut payload = format!(
        "\"shortlink\":\"{shortlink}\",\"longlink\":\"https://edited-{shortlink}.com\",\"reset_hits\":{reset_hits}"
    );
    if let Some(expiry) = expiry_time {
        payload.push_str(&format!(",\"expiry_time\":{expiry}"));
    }
    if let Some(note) = notes {
        payload.push_str(&format!(",\"notes\":\"{note}\""));
    }
    let req = Request::put("/api/edit")
        .header("X-API-Key", api_key)
        .body(Body::from(format!("{{{payload}}}")))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    resp.status()
}
