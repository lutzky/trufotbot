test:
    cargo test

release_frontend:
    trunk --config frontend build --release
    rm -rf server/assets/*
    cp -a frontend/dist/* server/assets/

serve_frontend_with_proxy:
    trunk serve --config frontend --proxy-backend=http://localhost:3000/api

serve_backend:
    cargo watch -q -cx 'run --bin trufotbot'
