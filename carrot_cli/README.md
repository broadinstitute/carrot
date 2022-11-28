![CARROT CLI](logo.png)
# carrot\_cli
The official CLI tool for [CARROT](https://github.com/broadinstitute/carrot). This tool provides a suite of commands for interacting with the CARROT REST API.

Current version: 0.6.2

## Table of Contents
* [Installation](#installation)
* [Setup](#setup)
* [Using carrot_cli](#using)
* [Development](#development)
    * [Linting files](#linting-files)
    * [Tests](#tests)
    * [Versioning](#versioning)

## <a name="installation">Installation</a>

    pip install .

## <a name="setup">Setup</a>
Using carrot_cli requires access to a running [CARROT](https://github.com/broadinstitute/carrot) server.  Once you have access to a server and have installed carrot_cli, there are a couple steps necessary for configuring the tool.
1. First, configure carrot_cli to point to your CARROT server using the following command:\
`> carrot_cli config set carrot_server_address <ADDRESS_OF_YOUR_CARROT_SERVER>`
2. Next, configure carrot_cli to use your email address to identify you.  This will associate any data you create in CARROT with your email address and allow you to receive notifications on the status of test runs.  Do this using the following command:\
`> carrot_cli config set email <YOUR_EMAIL>`

## <a name="using">Using carrot_cli</a>
If you are new to CARROT, start with the [User Guide](https://github.com/broadinstitute/carrot/blob/master/UserGuide.md), which provides an explanation of how CARROT works and how it should be used, along with examples using carrot_cli commands.

If you would like to start with a simple example that will allow you to run a test yourself, such an example exists in the [carrot-example-test](https://github.com/broadinstitute/carrot-example-test) repo.

## <a name="development">Development</a>

To do development in this codebase, the python3 development package must
be installed.

After installation the carrot\_cli development environment can be set up by
the following commands:

    python3 -mvenv venv
    . venv/bin/activate
    pip install -r dev-requirements.txt
    pip install -e .

### <a name="linting-files">Linting files</a>

    # run all linting commands
    tox -e lint

    # reformat all project files
    black src tests setup.py

    # sort imports in project files
    isort -rc src tests setup.py

    # check pep8 against all project files
    flake8 src tests setup.py

    # lint python code for common errors and codestyle issues
    pylint src

### <a name="tests">Tests</a>

    # run all linting and test
    tox

    # run only (fast) unit tests
    tox -e unit

    # run only linting
    tox -e lint

Note: If you run into "module not found" errors when running tox for testing, verify the modules are listed in test-requirements.txt and delete the .tox folder to force tox to refresh dependencies.
