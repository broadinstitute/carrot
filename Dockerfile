FROM rust:1.42.0 as builder
WORKDIR /usr/src/carrot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update \
    && apt-get -y --no-install-recommends install libpq-dev ca-certificates \
    && apt-get -y --no-install-recommends install git
COPY --from=builder /usr/local/cargo/bin/carrot /usr/local/bin/carrot
EXPOSE 80
CMD ["carrot"]