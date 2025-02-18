# dockerfile amd46

FROM rust:latest

RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev\
    librsvg2-dev \ 
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    libwebkit2gtk-4.1-dev \
    libclang-dev \
    clang \
    cmake \
    libclang1 \
    golang

    

# Install Node.js and npm first
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs

# Install pnpm using npm
RUN npm install -g pnpm

#RUN cargo install tauri-cli
RUN rustup target add x86_64-unknown-linux-gnu

 

# Set up working directory
WORKDIR /app
 
COPY . .

# Create SSL directory structure and copy SSL files
RUN mkdir -p /root/.local/share/com.rigidnetwork.sage/ssl/
COPY ./ssl/wallet.crt /root/.local/share/com.rigidnetwork.sage/ssl/
COPY ./ssl/wallet.key /root/.local/share/com.rigidnetwork.sage/ssl/

RUN cargo build --release -p sage-cli


 

EXPOSE 9257
#example curl command
#curl -k -v      --cert ~/.local/share/com.rigidnetwork.sage/ssl/wallet.crt      --key ~/.local/share/com.rigidnetwork.sage/ssl/wallet.key      -X POST https://localhost:9257/generate_mnemonic      -H "Content-Type: application/json"      -d '{"use_24_words":true}'

CMD  ["cargo", "run", "-p", "sage-cli", "--release", "--", "rpc", "start"]
#CMD ["/app/target/release/sage-cli", "rpc", "start"]