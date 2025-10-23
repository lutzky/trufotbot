# Stage 1: Frontend builder
# Builds the Vue.js frontend assets in a dedicated Node.js environment.
FROM node:22-alpine AS frontend_builder
WORKDIR /trufotbot/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm install
COPY frontend ./
RUN npm run build

# Stage 2: Rust builder base using cargo-chef
FROM rust:latest AS chef

RUN cargo install cargo-chef

WORKDIR /trufotbot

# Stage 3: Planner
# Creates the dependency recipe for caching.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json


# Stage 4: Main Builder
# Builds the final backend binary.
FROM chef AS builder
COPY --from=planner /trufotbot/recipe.json recipe.json

RUN sh -c "curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash" # cspell:disable-line
RUN cargo binstall just --version 1.40.0
RUN cargo binstall sqlx-cli --version 0.8.5

# Cook (build) the dependencies first for better caching
COPY --from=planner /trufotbot/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy the full application source
COPY . .

ENV DATABASE_URL=sqlite:build.db
RUN just reset_db

# Copy the built frontend assets from the frontend_builder stage.
# The Rust application will embed these assets using rust-embed.
COPY --from=frontend_builder /trufotbot/frontend/dist /trufotbot/server/assets

# Build the final, self-contained backend binary
RUN cargo build --release --bin trufotbot

# Stage 5: Final runtime image
FROM debian:bookworm-slim AS runtime
WORKDIR /trufotbot
# Install only necessary runtime dependencies
RUN apt-get update && \
	apt-get -y install libssl-dev ca-certificates tzdata && \
	rm -rf /var/lib/apt/lists/*

# Copy the final binary from the builder stage
COPY --from=builder /trufotbot/target/release/trufotbot /usr/local/bin

EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/trufotbot"]
CMD ["serve", "--host", "0.0.0.0"]
