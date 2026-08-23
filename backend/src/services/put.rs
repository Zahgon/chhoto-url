// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    AppState,
    auth::Auth,
    services::types::{
        ChhotoError::{ClientError, ServerError},
        JSONResponse,
    },
    utils,
};

// Edit a shortlink
pub(crate) async fn edit_link(auth: Auth, State(data): State<Arc<AppState>>, req: String) -> Response {
    let config = &data.config;
    match auth {
        Auth::ValidAPIKey | Auth::ValidSession => {
            match utils::edit_link_helper(&req, &*data.writer.lock().await, &data.hits_tx, config) {
                Ok(()) => {
                    let body = JSONResponse {
                        success: true,
                        error: false,
                        reason: String::from("Edit was successful."),
                    };
                    (StatusCode::CREATED, Json(body)).into_response()
                }
                Err(ServerError) => {
                    let body = JSONResponse {
                        success: false,
                        error: true,
                        reason: "Something went wrong when editing the link.".to_owned(),
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
                }
                Err(ClientError { reason }) => {
                    let body = JSONResponse {
                        success: false,
                        error: true,
                        reason,
                    };
                    (StatusCode::BAD_REQUEST, Json(body)).into_response()
                }
            }
        }
        Auth::None { result } | Auth::InvalidAPIKey { result } => {
            (StatusCode::UNAUTHORIZED, Json(result)).into_response()
        }
    }
}
