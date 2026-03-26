// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::api::responses;
use axum::Json;

pub const UTOIPA_TAG: &str = "status";

#[utoipa::path(
    get,
    path = "/api/status",
    summary = "Get server status",
    operation_id = "status_get",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Server status", body = responses::StatusResponse),
    ),
)]
pub async fn get() -> Json<responses::StatusResponse> {
    let now = chrono::Local::now();
    let tz = now.format("%Z").to_string();
    let server_time = now.format("%F %T").to_string();

    Json(responses::StatusResponse {
        timezone: tz,
        server_time,
    })
}
