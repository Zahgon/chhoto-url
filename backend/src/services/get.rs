// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};

use crate::{
    AppState,
    auth::Auth,
    database,
    services::types::{
        BackendConfig,
        ChhotoError::{ClientError, ServerError},
        GetReqParams,
    },
    utils,
};

// Return all active links
pub(crate) async fn getall(
    auth: Auth,
    State(data): State<Arc<AppState>>,
    Query(params): Query<GetReqParams>,
) -> Response {
    match auth {
        Auth::None { result: _ } => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
        Auth::InvalidAPIKey { result } => {
            (StatusCode::UNAUTHORIZED, result.reason).into_response()
        }
        _ => {
            let reader = data.reader.lock().await;
            match utils::getall_helper(&reader, params) {
                Ok(s) => (
                    [(header::CONTENT_TYPE, "application/json")],
                    s,
                )
                    .into_response(),
                Err(ServerError) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Something went wrong while loading the links.",
                )
                    .into_response(),
                Err(ClientError { reason }) => {
                    (StatusCode::BAD_REQUEST, reason).into_response()
                }
            }
        }
    }
}

// Get the site URL
// This is deprecated, and might be removed in the future.
// Use /api/getconfig instead
pub(crate) async fn siteurl(State(data): State<Arc<AppState>>) -> Response {
    if let Some(url) = &data.config.site_url {
        (StatusCode::OK, url.clone()).into_response()
    } else {
        (StatusCode::OK, "unset").into_response()
    }
}

// Get the version number
// This is deprecated, and might be removed in the future.
// Use /api/getconfig instead
pub(crate) async fn version() -> Response {
    (
        StatusCode::OK,
        format!("Chhoto URL v{}", utils::get_version()),
    )
        .into_response()
}

// Get the user's current role
pub(crate) async fn whoami(State(data): State<Arc<AppState>>, auth: Auth) -> Response {
    let config = &data.config;
    let acting_user = match auth {
        Auth::ValidAPIKey | Auth::ValidSession => "admin",
        _ => {
            if config.public_mode {
                "public"
            } else {
                "nobody"
            }
        }
    };
    (StatusCode::OK, acting_user).into_response()
}

// Get some useful backend config
pub(crate) async fn getconfig(auth: Auth, State(data): State<Arc<AppState>>) -> Response {
    let config = &data.config;
    let ok_response = || {
        let backend_config = BackendConfig {
            version: utils::get_version(),
            allow_capital_letters: config.allow_capital_letters,
            public_mode: config.public_mode,
            public_mode_expiry_delay: config.public_mode_expiry_delay.unwrap_or_default(),
            allowed_protocols: config.allowed_protocols.clone(),
            site_url: config.site_url.clone(),
            slug_style: config.slug_style.to_string(),
            slug_length: config.slug_length,
            try_longer_slug: config.try_longer_slug,
            frontend_page_size: config.frontend_page_size,
        };
        (StatusCode::OK, Json(backend_config)).into_response()
    };
    match auth {
        Auth::ValidSession | Auth::ValidAPIKey => ok_response(),
        Auth::None { result } | Auth::InvalidAPIKey { result } => {
            if data.config.public_mode {
                ok_response()
            } else {
                (StatusCode::UNAUTHORIZED, Json(result)).into_response()
            }
        }
    }
}

// Handle a given shortlink
pub(crate) async fn link_handler(
    Path(shortlink): Path<String>,
    State(data): State<Arc<AppState>>,
) -> Response {
    let reader = data.reader.lock().await;
    if let Ok(longlink) = database::find_and_add_hit(&shortlink, &reader, &data.hits_tx) {
        drop(reader);
        if data.config.use_temp_redirect {
            Redirect::temporary(&longlink).into_response()
        } else {
            // Defaults to permanent redirection
            Redirect::permanent(&longlink).into_response()
        }
    } else {
        drop(reader);
        utils::error404().await
    }
}
