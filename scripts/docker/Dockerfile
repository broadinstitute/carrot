FROM rust:1.42.0 as builder
WORKDIR /usr/src/carrot
COPY ./ .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update; \
    apt-get -y install libpq-dev;
COPY --from=builder /usr/local/cargo/bin/carrot /usr/local/bin/carrot
EXPOSE 8080
CMD ["carrot"]