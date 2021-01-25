FROM rust:1.42.0 as builder
WORKDIR /usr/src/carrot
COPY . .
RUN cargo install --path .

FROM debian:buster
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get -y --no-install-recommends install \
    libpq-dev \
    ca-certificates \
    git \
    openjdk-11-jre

ADD https://github.com/broadinstitute/cromwell/releases/download/55/womtool-55.jar /usr/local/bin/womtool/
ENV WOMTOOL_LOCATION=/usr/local/bin/womtool/
COPY --from=builder /usr/local/cargo/bin/carrot /usr/local/bin/carrot
EXPOSE 80
CMD ["carrot"]
