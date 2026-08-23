// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

use super::utils::*;

#[tokio::test]
async fn basic_site_config() {
    let test = "basic";
    let conf = default_config(test);
    let (_tempdir, app) = create_app(&conf, test).await;

    let resp = app
        .clone()
        .oneshot(Request::get("/api/siteurl").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, conf.site_url.clone().unwrap());

    let resp = app
        .clone()
        .oneshot(Request::get("/api/whoami").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "nobody");
    let resp = app
        .clone()
        .oneshot(
            Request::get("/api/whoami")
                .header("X-API-Key", conf.api_key.clone().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "admin");

    let resp = app
        .clone()
        .oneshot(Request::get("/api/version").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(
        body_string(resp)
            .await
            .starts_with(concat!("Chhoto URL v", env!("CARGO_PKG_VERSION")))
    );

    let resp = app
        .clone()
        .oneshot(
            Request::get("/api/getconfig")
                .header("X-API-Key", conf.api_key.unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let conf: BackendConfig = serde_json::from_str(&body_string(resp).await).unwrap();
    assert!(conf.version.starts_with(env!("CARGO_PKG_VERSION")));
    assert_eq!(conf.slug_length, 8);
}

#[tokio::test]
async fn auth_verification() {
    let test = "auth_verification";
    let conf = default_config(test);
    let (_tempdir, app) = create_app(&conf, test).await;

    let resp = app
        .clone()
        .oneshot(Request::get("/api/all").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    assert_eq!(body_string(resp).await, "Unauthorized");

    let status = edit_link(&app, "a", "test2", false, None, None).await;
    assert_eq!(status, 401);

    let (status, reply) = add_link(&app, "a", "test1", 0, "").await;
    assert_eq!(status, 401);
    assert_eq!(reply.reason, "API validation failed.");

    let resp = app
        .clone()
        .oneshot(
            Request::delete("/api/del/link")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    let resp = app
        .clone()
        .oneshot(Request::get("/api/getconfig").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}
