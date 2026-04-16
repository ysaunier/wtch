FROM rust:1.88-bookworm

# System dependencies for Tauri dev (Linux side: tests + compile checks)
RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libssl-dev \
    libayatana-appindicator3-dev \
    && rm -rf /var/lib/apt/lists/*

# Node.js LTS via NodeSource
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Tauri CLI
RUN cargo install tauri-cli --version "^2" --locked

WORKDIR /app

# Cache Rust dependencies: copy manifests first
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock* src-tauri/
RUN mkdir -p src-tauri/src && echo "fn main() {}" > src-tauri/src/main.rs \
    && cd src-tauri && cargo fetch || true

# Cache Node dependencies
COPY package.json package-lock.json* ./
RUN npm ci || true

COPY . .
