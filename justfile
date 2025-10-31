# Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only

# This help message
default:
    @just --list

# Run all tests
test target='':
    cargo test {{ target }}

# Run all tests in a loop
watch_test target='':
    cargo watch -cx "test {{ target }}"

release_frontend:
    cd frontend && npm run build
    rm -rf assets/*
    mkdir -p assets
    cp -a frontend/dist/* assets/

# Serve the frontend with a proxy to the backend (respects LISTEN_ADDRESS)
serve_frontend_with_proxy listen_address='':
    cd frontend && npm run dev \
        {{ if listen_address != '' { "-- --host " + listen_address } else { "" } }} \

# Serve the backend, restarting on changes
serve_backend:
    cargo watch -i frontend -q -x 'run --bin trufotbot serve'

# Serve both frontend and backend; Ctrl+C to stop both (respects LISTEN_ADDRESS)
serve_both listen_address='':
    #!/bin/bash
    trap cleanup EXIT
    cleanup() {
        pids="$(jobs -rp)"
        if [[ -n $pids ]]; then
            for pid in $pids; do
                kill -- -$pid
            done
        fi
    }
    setsid {{just_executable()}} serve_backend &
    setsid {{just_executable()}} serve_frontend_with_proxy {{listen_address}} &
    wait -n

set dotenv-load

db_basename := trim_start_match(env('DATABASE_URL', 'dev.db'), 'sqlite:')

seed_group_id := env_var_or_default('TELEGRAM_GROUP_ID', '')
seed_group_flag := if seed_group_id != '' { "-g=" + seed_group_id } else { "" }

# (re-)create the dev database
reset_db seed='':
    rm -f {{db_basename}} {{db_basename}}-wal {{db_basename}}-shm
    sqlx db reset -y
    {{ if seed == "seed" { \
        "cargo run --bin trufotbot -- seed " + seed_group_flag \
    } else { '' } }}

format:
    cargo fmt
    cd frontend && npm run format

api-update:
    cargo run --bin trufotbot -- schema > frontend/trufotbot-openapi.json
    cd frontend && npm run openapi-ts

frontend-check:
    cd frontend && \
        npm run lint && \
        npm run type-check && \
        npm run test:unit -- --run

docs-serve:
    . .venv/bin/activate && mkdocs serve

docs-build:
    . .venv/bin/activate && mkdocs build

docs-deploy:
    . .venv/bin/activate && mkdocs gh-deploy --force
