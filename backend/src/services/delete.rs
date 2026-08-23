// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use log::info;
use tower_sessions::Session;

use crate::{
    AppState,
    auth::Auth,
    services::types::{
        ChhotoError::{ClientError, ServerError},
        JSONResponse,
    },
    utils,
};

// Handle logout
// There's no reason to be calling this route with an API key
pub(crate) async fn logout(session: Session) -> Response {
    if matches!(session.remove::<String>("chhoto-url-auth").await, Ok(Some(_))) {
        info!("Successful logout.");
        (StatusCode::OK, "Logged out!").into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            "You don't seem to be logged in.",
        )
            .into_response()
    }
}

// Delete a given shortlink
pub(crate) async fn delete_link(
    Path(shortlink): Path<String>,
    auth: Auth,
    State(data): State<Arc<AppState>>,
) -> Response {
    match auth {
        Auth::ValidAPIKey => {
            match utils::delete_link_helper(
                &shortlink,
                &*data.writer.lock().await,
                data.config.allow_capital_letters,
            ) {
                Ok(()) => {
                    let response = JSONResponse {
                        success: true,
                        error: false,
                        reason: format!("Deleted {shortlink}"),
                    };
                    (StatusCode::OK, Json(response)).into_response()
                }
                Err(ServerError) => {
                    let response = JSONResponse {
                        success: false,
                        error: true,
                        reason: "Something went wrong when deleting the link.".to_owned(),
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
                }
                Err(ClientError { reason }) => {
                    let response = JSONResponse {
                        success: false,
                        error: true,
                        reason,
                    };
                    (StatusCode::NOT_FOUND, Json(response)).into_response()
                }
            }
        }
        Auth::InvalidAPIKey { result } => (StatusCode::UNAUTHORIZED, Json(result)).into_response(),
        // If using password - keeps backwards compatibility
        Auth::ValidSession => {
            if utils::delete_link_helper(
                &shortlink,
                &*data.writer.lock().await,
                data.config.allow_capital_letters,
            )
            .is_ok()
            {
                (StatusCode::OK, format!("Deleted {shortlink}")).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Not found!").into_response()
            }
        }
        Auth::None { result: _ } => (StatusCode::UNAUTHORIZED, "Not logged in!").into_response(),
    }
}
