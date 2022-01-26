FROM rust:1.55.0
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get -y --no-install-recommends install \
    libpq-dev \
    ca-certificates \
    git \
    openjdk-11-jre \
    wget

RUN wget -P /usr/local/bin/womtool https://github.com/broadinstitute/cromwell/releases/download/54/womtool-54.jar

ENV CARROT_WOMTOOL_LOCATION=/usr/local/bin/womtool/womtool-54.jar

WORKDIR /usr/src/carrot
COPY ./ .
CMD cargo test -- --test-threads=1
