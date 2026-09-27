FROM rust:1.91-bookworm AS build

WORKDIR /src
COPY . /src
RUN cargo build --release -p ramag-collaboration-relay

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 relay
RUN apt-get update && apt-get install --no-install-recommends --yes wget && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/ramag-collaboration-relay /usr/local/bin/ramag-collaboration-relay
USER relay
EXPOSE 18080
ENTRYPOINT ["/usr/local/bin/ramag-collaboration-relay"]
