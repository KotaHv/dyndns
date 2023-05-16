FROM ghcr.io/kotahv/dyndns-vue:latest as dyndns-vue

FROM rust:1.68-alpine as builder

RUN apk add --no-cache musl-dev pkgconfig openssl libressl-dev

RUN cargo new --bin /app
WORKDIR /app

COPY ./Cargo.* ./
RUN CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse cargo build --release \
    && find . -not -path "./target*" -delete

COPY . .
RUN touch src/main.rs
RUN CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse cargo build --release

FROM alpine:latest

RUN apk add --no-cache openssl

ENV DYNDNS_ADDR=0.0.0.0:80 DYNDNS_DEBUG=false DYNDNS_WEB_DIR=/dyndns-vue DYNDNS_DATABASE_URL=/dyndns/data/dyndns.db

VOLUME /dyndns/data
EXPOSE 80

WORKDIR /dyndns
COPY --from=builder /app/target/release/dyndns .
COPY --from=dyndns-vue /dyndns-vue /dyndns-vue

COPY docker/start.sh /start.sh

CMD [ "/start.sh" ]