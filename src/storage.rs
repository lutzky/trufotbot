// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Storage {
    pub pool: SqlitePool,
}
