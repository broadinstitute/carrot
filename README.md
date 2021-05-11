![CARROT](https://github.com/broadinstitute/carrot/blob/master/logo.png?raw=true)
# CARROT

This repository contains the Cromwell Automated Runner for Regression and Automation Testing.  This is a tool for configuring, running, and comparing the results of tests run in the [Cromwell Workflow Engine](https://github.com/broadinstitute/cromwell).

## Table of Contents
* [Requirements](#requirements)
    * [Building and Running CARROT](#building_and_running)
    * [Dynamic Software Testing](#software_building)
    * [Email Notifications](#email_notifications)
    * [GitHub Integration](#github_integration)
    * [Reporting](#reporting)
* [Style](#style)

## <a name="requirements">Requirements</a>

### <a name="building_and_running">Building and Running CARROT</a>
* A Rust version >=1.51.0 is required to build CARROT
    * rustup, the installer for Rust, can be found on the Rust website, [here](https://www.rust-lang.org/tools/install).
    * rustup will install the Rust compiler (rustc) and the Rust package manager (Cargo).
* CARROT currently requires a PostgreSQL database with version >=12.2 for storing test information.
    * PostgreSQL can be downloaded from the PostgreSQL website, [here](https://www.postgresql.org/download/).
    * It is also a requirement that the PostgreSQL DB have the `uuid-ossp` extension for using UUIDs.
        * This extension can be installed by connecting to the database as a user with SUPERUSER privileges and running the following command:
        `create extension if not exists "uuid-ossp";`
* Certain configuration information must be specified in environment variables before running.
    * These variables can be specified using a `.env` file.  An example of a `.env` configuration can be found within the `.env.example` file.
    * Alternatively, they can be specified normally as environment variables.  The `.env.example` file can be used for reference for the types and purposes of the various environment variables.
* CARROT uses the [Diesel](http://diesel.rs/) crate for interfacing with the database.  For certain dev and build tasks, the Diesel CLI is required.
    * Instructions for installing the Diesel CLI can be found [here](http://diesel.rs/guides/getting-started/).
    * Once the Diesel CLI is installed and the PostgreSQL database is running, the Diesel CLI migration tool can be used to create all of the required tables and types in the database with the command `diesel migration run`
    * Alternatively, these tables and types will all be created when running CARROT for the first time
* CARROT uses [womtool](https://cromwell.readthedocs.io/en/develop/WOMtool/) for WDL validation.  If running outside of a docker container created using the included Dockerfile, it will be necessary to include the womtool jar on the same machine and set the `WOMTOOL_LOCATION` environment variable to its location.
* Once Rust is installed, the project can be built using the `cargo build` command in the project directory.
    * Building for release can be done using `cargo build --release`
* CARROT requires a [Cromwell](https://github.com/broadinstitute/cromwell) server to run tests
    * Setting up a Cromwell server can be done by following the instructions [here](https://docs.google.com/document/d/1FlKe3XvjzE2-Yzi245THpC6X7D0opRufjh7Mt21bBhE/edit?usp=sharing)
* A Dockerfile is provided in the project root directory that can be used to run CARROT in a Docker container
    * The image can be built by running `docker build .` from the project root.
    * For development purposes, the `docker-compose.yml` file can be used to run CARROT with a PostreSQL server and a Cromwell server in their own containers.  This can be done using `docker-compose up` within that directory.
        * Environment variables must be set (following the `.env.example` file) within the `docker-compose.yml` file before running.
    * To run unit tests in Docker, use `docker-compose up --abort-on-container-exit --exit-code-from carrot-test` within the `/scripts/docker/test` directory.

### <a name="email_notifications">Email Notifications</a>
* CARROT supports the option of sending email notifications to subscribed users upon completion of a test run.  
    * Emails can be configured to be sent in the following ways:
        * Using the local machine's `sendmail` utility, or
        * Using an SMTP mail server (either running your own, or using an existing mail service like GMail).
    * Setting this up requires the use of a few configuration variables, which are listed and explained in the `.env.example` file.
* If you do not wish to utilize the email functionality, set the `EMAIL_MODE` environment variable to `None`.

### <a name="software_building">Dynamic Software Testing</a>
* It is possible (and encouraged) to set up CARROT to allow automatic generation of docker images for testing specific software hosted in a git repository
* In order to allow this for private GitHub repos, it is necessary to set the `ENABLE_PRIVATE_GITHUB_ACCESS` environment variable to true and fill in the related environment variables as detailed in the `.env.example` file 

### <a name="github_integration">GitHub Integration</a>
* CARROT supports triggering runs via GitHub PR comments, and receiving reply comments with run results.
* Enabling this functionality requires multiple steps:
    * Set up a [Google Cloud PubSub Topic](https://cloud.google.com/pubsub/docs/overview)
        * CARROT will use the created subscription to read messages to trigger runs from the topic
    * Create a GitHub account for CARROT to use to view and interact with GitHub
    * Add the [carrot-publish-github-action](https://github.com/broadinstitute/carrot-publish-github-action) to the GitHub Actions workflow for the repository you want to test
        * Instructions for doing so are included in the README for the action
    * Set the `ENABLE_GITHUB_REQUESTS` environment variable to true
        * Also set other related environment variables as detailed in the `.env.example` file

### <a name="reporting">Reporting</a>
* An important functionality of CARROT is the generation of reports from test runs in the form of Jupyter Notebooks
* In order for this functionality to work properly, it is necessary to:
    * Create a Google Cloud bucket for storing report templates and use it as the value for the `REPORT_LOCATION` environment variable
    * Build and push the report Dockerfile (`scripts/docker/reports/Dockerfile`) to a repository accessible by the Google Cloud service account associated with your Cromwell instance
        * Also set the `REPORT_DOCKER_LOCATION` environment variable to its location
        * Alternatively, you can build a docker image with Jupyter Notebook support and the libraries you need if the provided Dockerfile does not meet your needs

## <a name="style">Style</a>

When contributing to CARROT, you should do your best to adhere to the [Rust style guide](https://github.com/rust-dev-tools/fmt-rfcs/blob/master/guide/guide.md).

To make adhering to the style guide easier, there is a Rust automatic formatting tool called [rustfmt](https://github.com/rust-lang/rustfmt). This tool can be installed with cargo using the command `rustup component add rustfmt` and should be run using `cargo fmt` before making a pull request.
