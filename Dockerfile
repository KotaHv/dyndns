FROM ghcr.io/kotahv/dyndns-vue:latest AS dyndns-vue

FROM rust:1.68-alpine AS builder

RUN apk add --no-cache musl-dev

RUN cargo new --bin /app
WORKDIR /app

COPY ./Cargo.* ./
RUN CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse cargo build --release \
    && find . -not -path "./target*" -delete

COPY . .
RUN touch src/main.rs
RUN CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse cargo build --release

FROM alpine:latest

ENV DYNDNS_ADDR=0.0.0.0:80 DYNDNS_DEBUG=false DYNDNS_WEB_DIR=/dyndns-vue DYNDNS_DATABASE_URL=/dyndns/data/dyndns.db

VOLUME /dyndns/data
EXPOSE 80

WORKDIR /dyndns
COPY --from=builder /app/target/release/dyndns .
COPY --from=dyndns-vue /dyndns-vue /dyndns-vue

COPY docker/start.sh /start.sh

CMD [ "/start.sh" ]
