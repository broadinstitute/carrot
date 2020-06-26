# CARROT

This repository contains the Cromwell Automated Runner for Regression and Automation Testing.  This is a tool for configuring, running, and comparing the results of tests run in the [Cromwell Workflow Engine](https://github.com/broadinstitute/cromwell).

## Table of Contents
* [Requirements](#requirements)
* [Style](#style)

## <a name="requirements">Requirements</a>
* To build and run CARROT
    * A Rust version >=1.42.0 is required to build CARROT
        * rustup, the installer for Rust, can be found on the Rust website, [here](https://www.rust-lang.org/tools/install).
        * rustup will install the Rust compiler (rustc) and the Rust package manager (Cargo).
    * CARROT currently requires a PostgreSQL database with version >=12.2 for storing test information.
        * PostgreSQL can be downloaded from the PostgreSQL website, [here](https://www.postgresql.org/download/).
        * It is also a requirement that the PostgreSQL DB have the `uuid-ossp` extension for using UUIDs.
            * This extension can be installed by connecting to the database as a user with SUPERUSER privileges and running the following command:
            `create extension if not exists 'uuid-ossp';`
    * Certain configuration information must be specified in a file called `.env` before building.
        * An example of a `.env` configuration can be found within the `.env.example` file.
    * CARROT uses the [Diesel](http://diesel.rs/) crate for interfacing with the database.  For certain dev and build tasks, the Diesel CLI is required.
        * Instructions for installing the Diesel CLI can be found [here](http://diesel.rs/guides/getting-started/).
        * Once the Diesel CLI is installed and the PostgreSQL database is running, the Diesel CLI migration tool can be used to create all of the required tables and types in the database with the command `diesel migration run`
    * Once Rust is installed, the project can be built using the `cargo build` command in the project directory.
        * Building for release can be done using `cargo build --release`
* A Dockerfile is provided in the `/scripts/docker` directory that can be used to run CARROT in a Docker container
    * The image can be built by running `docker build -f scripts/docker/Dockerfile .` from the project root.
    * For development purposes, the `docker-compose.yml` file can be used to run CARROT with a PostreSQL server in another container as well.  This can be done using `docker-compose up` within that directory.
    * To run unit tests in Docker, use `docker-compose up --abort-on-container-exit --exit-code-from carrot-test` within the `/scripts/docker/test` directory.

## <a name="style">Style</a>

When contributing to CARROT, you should do your best to adhere to the [Rust style guide](https://github.com/rust-dev-tools/fmt-rfcs/blob/master/guide/guide.md).

To make adhering to the style guide easier, there is a Rust automatic formatting tool called [rustfmt](https://github.com/rust-lang/rustfmt). This tool can be installed with cargo using the command `rustup component add rustfmt` and should be run using `cargo fmt` before making a pull request.
