import json
import logging

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import reports, run_reports, runs


@pytest.fixture(autouse=True)
def unstub():
    yield
    mockito.unstub()


@pytest.fixture(autouse=True)
def no_email():
    mockito.when(config).load_var_no_error("email").thenReturn(None)

@pytest.fixture(
    params=[
        {
            "args": ["run", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "finished_at": None,
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "testsubmitted",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["run", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "title": "No run found",
                    "status": 404,
                    "detail": "No run found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(runs).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(runs).find_by_id(request.param["args"][2]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_by_id_data["args"])
    assert result.output == find_by_id_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": ["run", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "finished_at": "2020-09-16T18:58:06.371563",
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "evalfailed",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": ["run", "delete", "Sword of Protection run"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "finished_at": "2020-09-16T18:58:06.371563",
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "evalfailed",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "from_name": {
                "name": "Sword of Protection run",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "finished_at": "2020-09-16T18:58:06.371563",
                            "created_by": "adora@example.com",
                            "test_input": {"in_prev_owner": "Mara"},
                            "eval_input": {"in_creators": "Old Ones"},
                            "status": "evalfailed",
                            "results": {},
                            "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                            "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                            "name": "Sword of Protection run",
                            "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "email": "adora@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": ["run", "delete", "-y", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "finished_at": "2020-09-16T18:58:06.371563",
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "evalfailed",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": ["run", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "finished_at": "2020-09-16T18:58:06.371563",
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "evalfailed",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
            "interactive": {
                "input": "y",
                "message": "Run with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": ["run", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "finished_at": "2020-09-16T18:58:06.371563",
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "evalfailed",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Run with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["run", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "title": "No run found",
                    "status": 404,
                    "detail": "No run found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {
                    "title": "No run found",
                    "status": 404,
                    "detail": "No run found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def delete_data(request):
    # We want to load the value from "email" from config
    mockito.when(config).load_var("email").thenReturn(request.param["email"])
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(runs).delete(...).thenReturn(None)
    mockito.when(runs).find_by_id(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(runs).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(runs).delete(request.param["id"]).thenReturn(request.param["return"])
    mockito.when(runs).find_by_id(request.param["id"]).thenReturn(
        request.param["find_return"]
    )
    return request.param


def test_delete(delete_data, caplog):
    caplog.set_level(logging.INFO)
    runner = CliRunner()
    # Include interactive input and expected message if this test should trigger interactive stuff
    if "interactive" in delete_data:
        expected_output = (
            delete_data["interactive"]["message"] + delete_data["return"] + "\n"
        )
        result = runner.invoke(
            carrot, delete_data["args"], input=delete_data["interactive"]["input"]
        )
        assert result.output == expected_output
    else:
        result = runner.invoke(carrot, delete_data["args"])
        assert result.output == delete_data["return"] + "\n"
    # If we expect logging that we want to check, make sure it's there
    if "logging" in delete_data:
        assert delete_data["logging"] in caplog.text


@pytest.fixture(
    params=[
        {
            "args": [
                "run",
                "create_report",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--created_by",
                "adora@example.com",
                "--delete_failed",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "adora@example.com",
                True,
            ],
            "return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "created",
                    "cromwell_job_id": None,
                    "results": {},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": None,
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "run",
                "create_report",
                "Sword of Protection run",
                "New Sword of Protection report",
                "--created_by",
                "adora@example.com",
                "--delete_failed",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "adora@example.com",
                True,
            ],
            "from_names": {
                "report_name": "New Sword of Protection report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This new report replaced the broken one",
                            "name": "New Sword of Protection report",
                            "notebook": {
                                "metadata": {
                                    "language_info": {
                                        "codemirror_mode": {"name": "ipython", "version": 3},
                                        "file_extension": ".py",
                                        "mimetype": "text/x-python",
                                        "name": "python",
                                        "nbconvert_exporter": "python",
                                        "pygments_lexer": "ipython3",
                                        "version": "3.8.5-final",
                                    },
                                    "orig_nbformat": 2,
                                    "kernelspec": {
                                        "name": "python3",
                                        "display_name": "Python 3.8.5 64-bit",
                                        "metadata": {
                                            "interpreter": {
                                                "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                                            }
                                        },
                                    },
                                },
                                "nbformat": 4,
                                "nbformat_minor": 2,
                                "cells": [
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message = carrot_run_data["results"]["Greeting"]\n',
                                            "print(message)",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message_file = open(carrot_downloads["results"]["File Result"], \'r\')\n',
                                            "print(message_file.read())",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": ["print('Thanks')"],
                                    },
                                ],
                            },
                            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
                "run_name": "Sword of Protection run",
                "run_return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "finished_at": "2020-09-16T18:58:06.371563",
                            "created_by": "adora@example.com",
                            "test_input": {"in_prev_owner": "Mara"},
                            "eval_input": {"in_creators": "Old Ones"},
                            "status": "succeeded",
                            "results": {},
                            "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                            "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                            "name": "Sword of Protection run",
                            "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "created",
                    "cromwell_job_id": None,
                    "results": {},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": None,
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "run",
                "create_report",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": ["run", "create_report"],
            "params": [],
            "return": "Usage: carrot_cli run create_report [OPTIONS] RUN REPORT\n"
            "Try 'carrot_cli run create_report -h' for help.\n"
            "\n"
            "Error: Missing argument 'RUN'.",
        },
    ]
)
def create_report_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(run_reports).create_map(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(runs).find(
            name=request.param["from_names"]["run_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["run_return"])
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(run_reports).create_map(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
        ).thenReturn(request.param["return"])
    return request.param


def test_create_report(create_report_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, create_report_data["args"])
    if "logging" in create_report_data:
        assert create_report_data["logging"] in caplog.text
    else:
        assert result.output == create_report_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "run",
                "find_report_by_ids",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "run",
                "find_report_by_ids",
                "Sword of Protection run",
                "New Sword of Protection report",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "from_names": {
                "report_name": "New Sword of Protection report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This new report replaced the broken one",
                            "name": "New Sword of Protection report",
                            "notebook": {
                                "metadata": {
                                    "language_info": {
                                        "codemirror_mode": {"name": "ipython", "version": 3},
                                        "file_extension": ".py",
                                        "mimetype": "text/x-python",
                                        "name": "python",
                                        "nbconvert_exporter": "python",
                                        "pygments_lexer": "ipython3",
                                        "version": "3.8.5-final",
                                    },
                                    "orig_nbformat": 2,
                                    "kernelspec": {
                                        "name": "python3",
                                        "display_name": "Python 3.8.5 64-bit",
                                        "metadata": {
                                            "interpreter": {
                                                "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                                            }
                                        },
                                    },
                                },
                                "nbformat": 4,
                                "nbformat_minor": 2,
                                "cells": [
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message = carrot_run_data["results"]["Greeting"]\n',
                                            "print(message)",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message_file = open(carrot_downloads["results"]["File Result"], \'r\')\n',
                                            "print(message_file.read())",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": ["print('Thanks')"],
                                    },
                                ],
                            },
                            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
                "run_name": "Sword of Protection run",
                "run_return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "finished_at": "2020-09-16T18:58:06.371563",
                            "created_by": "adora@example.com",
                            "test_input": {"in_prev_owner": "Mara"},
                            "eval_input": {"in_creators": "Old Ones"},
                            "status": "succeeded",
                            "results": {},
                            "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                            "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                            "name": "Sword of Protection run",
                            "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["run", "find_report_by_ids"],
            "params": [],
            "return": "Usage: carrot_cli run find_report_by_ids [OPTIONS] RUN REPORT\n"
            "Try 'carrot_cli run find_report_by_ids -h' for help.\n"
            "\n"
            "Error: Missing argument 'RUN'.",
        },
    ]
)
def find_report_by_ids_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(run_reports).find_map_by_ids(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(runs).find(
            name=request.param["from_names"]["run_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["run_return"])
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(run_reports).find_map_by_ids(
            request.param["params"][0],
            request.param["params"][1],
        ).thenReturn(request.param["return"])
    return request.param


def test_find_report_by_ids(find_report_by_ids_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_report_by_ids_data["args"])
    assert result.output == find_report_by_ids_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "run",
                "find_reports",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--report_id",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--status",
                "succeeded",
                "--cromwell_job_id",
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "--results",
                "tests/data/mock_report_results.json",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--finished_before",
                "2020-10-00T00:00:00.000000",
                "--finished_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(status)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "succeeded",
                "d9855002-6b71-429c-a4de-8e90222488cd",
                {"result1": "val1"},
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "adora@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(status)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "status": "succeeded",
                        "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "results": {"result1": "val1"},
                        "created_at": "2020-09-24T19:07:59.311462",
                        "created_by": "adora@example.com",
                        "finished_at": "2020-09-24T21:07:59.311462",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "run",
                "find_reports",
                "Sword of Protection run",
                "--report",
                "New Sword of Protection report",
                "--status",
                "succeeded",
                "--cromwell_job_id",
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "--results",
                "tests/data/mock_report_results.json",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--finished_before",
                "2020-10-00T00:00:00.000000",
                "--finished_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(status)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "succeeded",
                "d9855002-6b71-429c-a4de-8e90222488cd",
                {"result1": "val1"},
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "adora@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(status)",
                1,
                0,
            ],
            "from_names": {
                "report_name": "New Sword of Protection report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This new report replaced the broken one",
                            "name": "New Sword of Protection report",
                            "notebook": {
                                "metadata": {
                                    "language_info": {
                                        "codemirror_mode": {"name": "ipython", "version": 3},
                                        "file_extension": ".py",
                                        "mimetype": "text/x-python",
                                        "name": "python",
                                        "nbconvert_exporter": "python",
                                        "pygments_lexer": "ipython3",
                                        "version": "3.8.5-final",
                                    },
                                    "orig_nbformat": 2,
                                    "kernelspec": {
                                        "name": "python3",
                                        "display_name": "Python 3.8.5 64-bit",
                                        "metadata": {
                                            "interpreter": {
                                                "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                                            }
                                        },
                                    },
                                },
                                "nbformat": 4,
                                "nbformat_minor": 2,
                                "cells": [
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message = carrot_run_data["results"]["Greeting"]\n',
                                            "print(message)",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message_file = open(carrot_downloads["results"]["File Result"], \'r\')\n',
                                            "print(message_file.read())",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": ["print('Thanks')"],
                                    },
                                ],
                            },
                            "report_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
                "run_name": "Sword of Protection run",
                "run_return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "finished_at": "2020-09-16T18:58:06.371563",
                            "created_by": "adora@example.com",
                            "test_input": {"in_prev_owner": "Mara"},
                            "eval_input": {"in_creators": "Old Ones"},
                            "status": "succeeded",
                            "results": {},
                            "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                            "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                            "name": "Sword of Protection run",
                            "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                [
                    {
                        "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "status": "succeeded",
                        "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "results": {"result1": "val1"},
                        "created_at": "2020-09-24T19:07:59.311462",
                        "created_by": "adora@example.com",
                        "finished_at": "2020-09-24T21:07:59.311462",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "run",
                "find_reports",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No run_reports found",
                    "status": 404,
                    "detail": "No run_reports found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_reports_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(run_reports).find_maps(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(runs).find(
            name=request.param["from_names"]["run_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["run_return"])
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
    # Mock up request response
    mockito.when(run_reports).find_maps(
        request.param["params"][0],
        request.param["params"][1],
        request.param["params"][2],
        request.param["params"][3],
        request.param["params"][4],
        request.param["params"][5],
        request.param["params"][6],
        request.param["params"][7],
        request.param["params"][8],
        request.param["params"][9],
        request.param["params"][10],
        request.param["params"][11],
        request.param["params"][12],
    ).thenReturn(request.param["return"])
    return request.param


def test_find_reports(find_reports_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_reports_data["args"])
    assert result.output == find_reports_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "run",
                "delete_report_by_ids",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "run",
                "delete_report_by_ids",
                "Sword of Protection run",
                "New Sword of Protection report",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
            "from_names": {
                "report_name": "New Sword of Protection report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This new report replaced the broken one",
                            "name": "New Sword of Protection report",
                            "notebook": {
                                "metadata": {
                                    "language_info": {
                                        "codemirror_mode": {"name": "ipython", "version": 3},
                                        "file_extension": ".py",
                                        "mimetype": "text/x-python",
                                        "name": "python",
                                        "nbconvert_exporter": "python",
                                        "pygments_lexer": "ipython3",
                                        "version": "3.8.5-final",
                                    },
                                    "orig_nbformat": 2,
                                    "kernelspec": {
                                        "name": "python3",
                                        "display_name": "Python 3.8.5 64-bit",
                                        "metadata": {
                                            "interpreter": {
                                                "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                                            }
                                        },
                                    },
                                },
                                "nbformat": 4,
                                "nbformat_minor": 2,
                                "cells": [
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message = carrot_run_data["results"]["Greeting"]\n',
                                            "print(message)",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": [
                                            'message_file = open(carrot_downloads["results"]["File Result"], \'r\')\n',
                                            "print(message_file.read())",
                                        ],
                                    },
                                    {
                                        "cell_type": "code",
                                        "execution_count": None,
                                        "metadata": {},
                                        "outputs": [],
                                        "source": ["print('Thanks')"],
                                    },
                                ],
                            },
                            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
                "run_name": "Sword of Protection run",
                "run_return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "finished_at": "2020-09-16T18:58:06.371563",
                            "created_by": "adora@example.com",
                            "test_input": {"in_prev_owner": "Mara"},
                            "eval_input": {"in_creators": "Old Ones"},
                            "status": "succeeded",
                            "results": {},
                            "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                            "eval_cromwell_job_id": "39482203-6b71-429c-a4de-8e90222488cd",
                            "name": "Sword of Protection run",
                            "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "email": "adora@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "run",
                "delete_report_by_ids",
                "-y",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "run",
                "delete_report_by_ids",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
            "interactive": {
                "input": "y",
                "message": "Mapping for run with id cd987859-06fe-4b1a-9e96-47d4f36bf819 and report with id "
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8 was created by adora@example.com. Are you sure you "
                "want to delete? [y/N]: y\n",
            },
        },
        {
            "args": [
                "run",
                "delete_report_by_ids",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Mapping for run with id cd987859-06fe-4b1a-9e96-47d4f36bf819 and report with id "
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8 was created by adora@example.com. Are you sure you "
                "want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["run", "delete_report_by_ids"],
            "ids": [],
            "find_return": json.dumps(
                {
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": "Usage: carrot_cli run delete_report_by_ids [OPTIONS] RUN REPORT\n"
            "Try 'carrot_cli run delete_report_by_ids -h' for help.\n"
            "\n"
            "Error: Missing argument 'RUN'.",
        },
    ]
)
def delete_report_by_ids_data(request):
    # We want to load the value from "email" from config
    mockito.when(config).load_var("email").thenReturn(request.param["email"])
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(run_reports).delete_map_by_ids(...).thenReturn(None)
    mockito.when(run_reports).find_map_by_ids(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(runs).find(
            name=request.param["from_names"]["run_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["run_return"])
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["ids"]) > 0:
        mockito.when(run_reports).delete_map_by_ids(
            request.param["ids"][0],
            request.param["ids"][1],
        ).thenReturn(request.param["return"])
        mockito.when(run_reports).find_map_by_ids(
            request.param["ids"][0],
            request.param["ids"][1],
        ).thenReturn(request.param["find_return"])
    return request.param


def test_delete_report_by_ids(delete_report_by_ids_data, caplog):
    caplog.set_level(logging.INFO)
    runner = CliRunner()
    # Include interactive input and expected message if this test should trigger interactive stuff
    if "interactive" in delete_report_by_ids_data:
        expected_output = (
            delete_report_by_ids_data["interactive"]["message"]
            + delete_report_by_ids_data["return"]
            + "\n"
        )
        result = runner.invoke(
            carrot,
            delete_report_by_ids_data["args"],
            input=delete_report_by_ids_data["interactive"]["input"],
        )
        assert result.output == expected_output
    else:
        result = runner.invoke(carrot, delete_report_by_ids_data["args"])
        assert result.output == delete_report_by_ids_data["return"] + "\n"
    # If we expect logging that we want to check, make sure it's there
    if "logging" in delete_report_by_ids_data:
        assert delete_report_by_ids_data["logging"] in caplog.text
