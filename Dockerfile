FROM rust:1.51.0 as builder
WORKDIR /usr/src/carrot
COPY . .
RUN cargo install --path .

FROM debian:buster-20210408
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get -y --no-install-recommends install \
    libpq-dev \
    ca-certificates \
    git \
    openjdk-11-jre \
    wget

RUN wget -P /usr/local/bin/womtool https://github.com/broadinstitute/cromwell/releases/download/54/womtool-54.jar

ENV WOMTOOL_LOCATION=/usr/local/bin/womtool/womtool-54.jar
COPY --from=builder /usr/local/cargo/bin/carrot /usr/local/bin/carrot
EXPOSE 80
WORKDIR /carrot_root
CMD ["carrot"]
