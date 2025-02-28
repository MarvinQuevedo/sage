# dockerfile amd64

FROM rust:slim-bookworm AS builder

# Update and install only necessary packages
RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    libclang-dev \
    clang \
    cmake \
    dnsutils \
    ca-certificates \
    openssl \
    net-tools \
    iputils-ping \
    iproute2 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install specific golang version 1.22.4 (updated from 1.21.8 to fix security vulnerability)
RUN wget https://golang.org/dl/go1.22.4.linux-amd64.tar.gz && \
    tar -C /usr/local -xzf go1.22.4.linux-amd64.tar.gz && \
    rm go1.22.4.linux-amd64.tar.gz
ENV PATH=$PATH:/usr/local/go/bin

# Install Node.js and npm first (using the official Node.js repository)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get update && apt-get install -y nodejs && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Verify Node.js version
RUN node --version

# Install pnpm using npm
RUN npm install -g pnpm

# Set up Rust with specific version
RUN rustup target add x86_64-unknown-linux-gnu && \
    rustup update && \
    rustup default stable && \
    rustup toolchain install 1.81.0 && \
    rustup default 1.81.0

# Create a non-root user and group
RUN groupadd -r appuser && useradd -r -g appuser -m appuser

# Set up working directory
WORKDIR /app

# Copy only necessary files first to optimize build caching
COPY --chown=appuser:appuser Cargo.* ./
COPY --chown=appuser:appuser rust-toolchain.toml ./
# Create directory structure for source files
RUN mkdir -p src/bin

# Copy the rest of the application
COPY --chown=appuser:appuser . .

# Create SSL directory structure and copy SSL files
RUN mkdir -p /home/appuser/.local/share/com.rigidnetwork.sage/ssl/ && \
    cp ./ssl/wallet.crt /home/appuser/.local/share/com.rigidnetwork.sage/ssl/ && \
    cp ./ssl/wallet.key /home/appuser/.local/share/com.rigidnetwork.sage/ssl/ && \
    chown -R appuser:appuser /home/appuser/.local

# Build the application
RUN cargo build --release -p sage-cli && \
    # Fix permissions for the built application
    chown -R appuser:appuser /app/target

# Configure system for better TLS handling
RUN update-ca-certificates

# Apply security updates
RUN apt-get update && apt-get upgrade -y && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Set environment variables
ENV RUST_LOG=debug

# Expose port
EXPOSE 9257/tcp

#example curl command
#curl -k -v      --cert ~/.local/share/com.rigidnetwork.sage/ssl/wallet.crt      --key ~/.local/share/com.rigidnetwork.sage/ssl/wallet.key      -X POST https://localhost:9257/generate_mnemonic      -H "Content-Type: application/json"      -d '{"use_24_words":true}'

# Switch to non-root user
USER appuser

CMD ["cargo", "run", "-p", "sage-cli", "--release", "--", "rpc", "start"]
#CMD ["/app/target/release/sage-cli", "rpc", "start"]