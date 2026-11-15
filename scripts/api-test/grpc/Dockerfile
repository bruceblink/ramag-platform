FROM rust:1.100.0-bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea AS builder

WORKDIR /src
RUN sed -i 's/ bookworm-updates//g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update -o Acquire::Retries=5 \
    && apt-get install --yes --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251

RUN useradd --system --uid 10001 --create-home api-test
COPY --from=builder /src/target/release/ramag-api-grpc-test /usr/local/bin/ramag-api-grpc-test
USER api-test
EXPOSE 50051
HEALTHCHECK --interval=2s --timeout=3s --retries=20 CMD ["/bin/bash", "-c", "</dev/tcp/127.0.0.1/50051"]
ENTRYPOINT ["/usr/local/bin/ramag-api-grpc-test"]
