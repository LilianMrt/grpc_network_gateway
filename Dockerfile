# --- build -------------------------------------------------------------------
FROM rust:1.97-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3 AS builder

WORKDIR /build

# protobuf-compiler: build.rs generates the tonic stubs from proto/gateway.proto.
# The rest is what openssl's `vendored` feature needs to compile from source.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        protobuf-compiler pkg-config perl make \
    && rm -rf /var/lib/apt/lists/*

# Build dependencies against a stub crate first, so editing application source
# does not rebuild the dependency tree.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY . .

# SQLX_OFFLINE makes the query! macros read .sqlx/ instead of opening a
# connection, so the image builds with no database reachable.
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked --bin grpc_network_gateway \
    && strip target/release/grpc_network_gateway

# --- runtime -----------------------------------------------------------------
# distroless/cc: no shell and no package manager, but the glibc and libgcc the
# binary is dynamically linked against. The :nonroot variant runs as uid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f

COPY --from=builder /build/target/release/grpc_network_gateway /usr/local/bin/gateway

EXPOSE 50051
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/gateway"]
