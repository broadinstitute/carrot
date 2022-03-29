import json
import logging

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import reports


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
            "args": ["report", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This report will save Etheria",
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
                    "config": {"cpu": 2},
                    "name": "Sword of Protection report",
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["report", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "title": "No report found",
                    "status": 404,
                    "detail": "No report found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(reports).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(reports).find_by_id(request.param["args"][2]).thenReturn(
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
            "args": [
                "report",
                "find",
                "--report_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Sword of Protection report",
                "--description",
                "This report will save Etheria",
                "--notebook",
                "tests/data/mock_report_notebook.ipynb",
                "--config",
                "tests/data/mock_report_config.json",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(name)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "Sword of Protection report",
                "This report will save Etheria",
                {
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
                {"cpu": 2},
                "adora@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This report will save Etheria",
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
                    "config": {"cpu": 2},
                    "name": "Sword of Protection report",
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "report",
                "find",
                "--report_id",
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
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No reports found",
                    "status": 404,
                    "detail": "No reports found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(reports).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(reports).find(
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
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_data["args"])
    assert result.output == find_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "report",
                "create",
                "--name",
                "Sword of Protection report",
                "--description",
                "This report will save Etheria",
                "--notebook",
                "tests/data/mock_report_notebook.ipynb",
                "--config",
                "tests/data/mock_report_config.json",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection report",
                "This report will save Etheria",
                {
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
                {"cpu": 2},
                "adora@example.com",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This report will save Etheria",
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
                    "config": {"cpu": 2},
                    "name": "Sword of Protection report",
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "report",
                "create",
                "--name",
                "Sword of Protection report",
                "--description",
                "This report will save Etheria",
                "--notebook",
                "tests/data/mock_report_notebook.ipynb",
                "--config",
                "tests/data/mock_report_config.json",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": ["report", "create"],
            "params": [],
            "return": "Usage: carrot_cli report create [OPTIONS]\n"
            "Try 'carrot_cli report create -h' for help.\n"
            "\n"
            "Error: Missing option '--name'.",
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(reports).create(...).thenReturn(None)
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(reports).create(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
            request.param["params"][4],
        ).thenReturn(request.param["return"])
    return request.param


def test_create(create_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, create_data["args"])
    if "logging" in create_data:
        assert create_data["logging"] in caplog.text
    else:
        assert result.output == create_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "report",
                "update",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--description",
                "This new report replaced the broken one",
                "--name",
                "New Sword of Protection report",
                "--notebook",
                "tests/data/mock_report_notebook.ipynb",
                "--config",
                "tests/data/mock_report_config.json",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection report",
                "This new report replaced the broken one",
                {
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
                {"cpu": 2},
            ],
            "return": json.dumps(
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
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
{
            "args": [
                "report",
                "update",
                "Old Sword of Protection report",
                "--description",
                "This new report replaced the broken one",
                "--name",
                "New Sword of Protection report",
                "--notebook",
                "tests/data/mock_report_notebook.ipynb",
                "--config",
                "tests/data/mock_report_config.json",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection report",
                "This new report replaced the broken one",
                {
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
                {"cpu": 2},
            ],
            "from_name": {
                "name": "Old Sword of Protection report",
                "return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This old report is old",
                            "name": "Old Sword of Protection report",
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
                            "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
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
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["report", "update"],
            "params": [],
            "return": "Usage: carrot_cli report update [OPTIONS] REPORT\n"
            "Try 'carrot_cli report update -h' for help.\n"
            "\n"
            "Error: Missing argument 'REPORT'.",
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(reports).update(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(reports).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(reports).update(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
            request.param["params"][4],
        ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    runner = CliRunner()
    result = runner.invoke(carrot, update_data["args"])
    assert result.output == update_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": ["report", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
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
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                }
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": ["report", "delete", "-y", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
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
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                }
            ),
            "email": "catra@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": ["report", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
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
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "message": "Report with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": ["report", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
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
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Report with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["report", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "email": "catra@example.com",
            "find_return": json.dumps(
                {
                    "title": "No report found",
                    "status": 404,
                    "detail": "No report found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
            "return": json.dumps(
                {
                    "title": "No report found",
                    "status": 404,
                    "detail": "No report found with the specified ID",
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
    mockito.when(reports).delete(...).thenReturn(None)
    mockito.when(reports).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(reports).delete(request.param["id"]).thenReturn(
        request.param["return"]
    )
    mockito.when(reports).find_by_id(request.param["id"]).thenReturn(
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
