// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

mod delete;
mod get;
mod post;
mod put;
pub(crate) mod types;
pub(super) mod utils;

use std::sync::Arc;

use axum::{
    Router,
    handler::HandlerWithoutStateExt,
    routing::{delete as delete_method, get as get_method, post, put},
};
use http::header;
use tower_http::{
    compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tower_sessions::{
    Expiry, MemoryStore, SessionManagerLayer,
    cookie::{SameSite, time::Duration},
};

use crate::{AppState, config::Config};

// Build the Axum router with all routes, middleware layers and shared state.
pub(crate) fn build_router(state: Arc<AppState>, conf: &Config) -> Router {
    // Session middleware: signed, in-memory cookie store.
    // Sessions are invalidated on server restart (matches previous behavior of
    // generating a fresh signing key on each boot).
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_same_site(SameSite::Strict)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(7)));

    let mut router = Router::new()
        .route("/api/all", get_method(get::getall))
        .route("/api/siteurl", get_method(get::siteurl))
        .route("/api/version", get_method(get::version))
        .route("/api/whoami", get_method(get::whoami))
        .route("/api/getconfig", get_method(get::getconfig))
        .route("/api/new", post(post::add_links))
        .route("/api/expand", post(post::expand))
        .route("/api/login", post(post::login))
        .route("/api/edit", put(put::edit_link))
        .route("/api/logout", delete_method(delete::logout))
        .route("/api/del/{shortlink}", delete_method(delete::delete_link))
        .route("/{shortlink}", get_method(get::link_handler));

    // Serve the frontend as static files unless disabled.
    if !conf.disable_frontend {
        if conf.custom_landing_directory.is_some() {
            router = router.nest_service(
                "/admin/manage",
                ServeDir::new("./frontend").append_index_html_on_directories(true),
            );
        }
        let landing = conf
            .custom_landing_directory
            .clone()
            .unwrap_or_else(|| "./frontend".to_owned());
        router = router.fallback_service(
            ServeDir::new(landing)
                .append_index_html_on_directories(true)
                .not_found_service(utils::error404.into_service()),
        );
    } else {
        router = router.fallback(utils::error404);
    }

    let mut router = router
        .layer(session_layer)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    // Optional Cache-Control header, only added when not already present.
    if let Some(header_value) = &conf.cache_control_header
        && let Ok(value) = header::HeaderValue::from_str(header_value)
    {
        router = router.layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            value,
        ));
    }

    router.with_state(state)
}
