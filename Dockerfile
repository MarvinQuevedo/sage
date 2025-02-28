# dockerfile amd64

FROM rust:latest

RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    libwebkit2gtk-4.1-dev \
    libclang-dev \
    clang \
    cmake \
    libclang1 \
    dnsutils \
    ca-certificates \
    openssl \
    net-tools \
    iputils-ping \
    iproute2

# Install specific golang version 1.19.8
RUN wget https://golang.org/dl/go1.19.8.linux-amd64.tar.gz && \
    tar -C /usr/local -xzf go1.19.8.linux-amd64.tar.gz && \
    rm go1.19.8.linux-amd64.tar.gz
ENV PATH=$PATH:/usr/local/go/bin

# Install Node.js and npm first
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs

# Install pnpm using npm
RUN npm install -g pnpm

#RUN cargo install tauri-cli
RUN rustup target add x86_64-unknown-linux-gnu
RUN rustup update

# Specify exact Rust version and prevent auto-updates
RUN rustup default stable
RUN rustup toolchain install 1.81.0
RUN rustup default 1.81.0

# Set up working directory
WORKDIR /app

COPY . .

# Create SSL directory structure and copy SSL files
RUN mkdir -p /root/.local/share/com.rigidnetwork.sage/ssl/
COPY ./ssl/wallet.crt /root/.local/share/com.rigidnetwork.sage/ssl/
COPY ./ssl/wallet.key /root/.local/share/com.rigidnetwork.sage/ssl/


RUN cargo build --release -p sage-cli

# Create a non-root user and group
RUN groupadd -r appuser && useradd -r -g appuser appuser

# Create SSL directory for the non-root user
RUN mkdir -p /home/appuser/.local/share/com.rigidnetwork.sage/ssl/
COPY ./ssl/wallet.crt /home/appuser/.local/share/com.rigidnetwork.sage/ssl/
COPY ./ssl/wallet.key /home/appuser/.local/share/com.rigidnetwork.sage/ssl/

# Set proper ownership
RUN chown -R appuser:appuser /app
RUN chown -R appuser:appuser /home/appuser

ENV RUST_LOG=debug

# Expose both IPv4 and IPv6 ports
EXPOSE 9257/tcp

#example curl command
#curl -k -v      --cert ~/.local/share/com.rigidnetwork.sage/ssl/wallet.crt      --key ~/.local/share/com.rigidnetwork.sage/ssl/wallet.key      -X POST https://localhost:9257/generate_mnemonic      -H "Content-Type: application/json"      -d '{"use_24_words":true}'

# Configure system for better TLS handling
RUN update-ca-certificates

# Apply security updates to address CVE-2025-27091
RUN apt-get update && apt-get upgrade -y

# Switch to non-root user
USER appuser

CMD ["cargo", "run", "-p", "sage-cli", "--release", "--", "rpc", "start"]
#CMD ["/app/target/release/sage-cli", "rpc", "start"]