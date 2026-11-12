# Static musl build; ring and webpki-roots need no system TLS or CA files.
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev gcc
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked && strip target/release/cinesharebot

FROM scratch
COPY --from=build /src/target/release/cinesharebot /app
USER 10001:10001
EXPOSE 3000
ENTRYPOINT ["/app"]
