# This help message
default:
    @just --list

# Run all tests
test:
    cargo test

# Run all tests in a loop
watch_test:
    cargo watch -cx test

release_frontend:
    trunk --config frontend build --cargo-profile wasm-release --release
    rm -rf server/assets/*
    cp -a frontend/dist/* server/assets/

# Serve the frontend with a proxy to the backend (respects LISTEN_ADDRESS)
serve_frontend_with_proxy listen_address='':
    trunk serve --config frontend \
        {{ if listen_address != '' { "-a " + listen_address } else { "" } }} \
        --proxy-backend=http://localhost:3000/api

# Serve the backend, restarting on changes
serve_backend:
    cargo watch -i frontend -q -cx 'run --bin trufotbot'

# Serve both frontend and backend; Ctrl+C to stop both (respects LISTEN_ADDRESS)
serve_both listen_address='':
    #!/bin/bash
    trap 'kill %1' EXIT
    {{just_executable()}} serve_backend &
    {{just_executable()}} serve_frontend_with_proxy {{listen_address}}

set dotenv-load

db_basename := trim_start_match(env('DATABASE_URL'), 'sqlite:')

# (re-)create the dev database
reset_db:
    rm -f {{db_basename}} {{db_basename}}-wal {{db_basename}}-shm
    cd server && sqlx db reset -y
    mv server/{{db_basename}} .
    cargo run --bin trufotbot -- --seed
