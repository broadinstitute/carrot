FROM rust:1.59.0-buster as builder
WORKDIR /usr/src/carrot
COPY . .
RUN cargo install --path .

FROM debian:buster-20220912
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get -y --no-install-recommends install \
    libpq-dev \
    ca-certificates \
    git \
    openjdk-11-jre \
    wget \
    libc6

RUN wget -P /usr/local/bin/womtool https://github.com/broadinstitute/cromwell/releases/download/54/womtool-54.jar

ENV CARROT_WOMTOOL_LOCATION=/usr/local/bin/womtool/womtool-54.jar
ENV CARROT_WDL_DIRECTORY=/carrot/wdl
COPY --from=builder /usr/local/cargo/bin/carrot /usr/local/bin/carrot
EXPOSE 80
WORKDIR /carrot_root
CMD ["carrot"]