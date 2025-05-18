test:
    cargo test

release_frontend:
    trunk --config frontend build --cargo-profile wasm-release --release
    rm -rf server/assets/*
    cp -a frontend/dist/* server/assets/

serve_frontend_with_proxy:
    trunk serve --config frontend --proxy-backend=http://localhost:3000/api

serve_backend:
    cargo watch -i frontend -q -cx 'run --bin trufotbot'

set dotenv-load

db_basename := trim_start_match(env('DATABASE_URL'), 'sqlite:')

reset_db:
    rm -f {{db_basename}} {{db_basename}}-wal {{db_basename}}-shm
    cd server && sqlx db reset -y
    mv server/{{db_basename}} .
    cargo run --bin trufotbot -- --seed
