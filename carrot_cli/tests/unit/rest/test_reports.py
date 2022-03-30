import json

import mockito
import pytest
from carrot_cli.rest import reports, request_handler


@pytest.fixture(autouse=True)
def unstub():
    yield
    mockito.unstub()


@pytest.fixture(
    params=[
        {
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
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
                    "description": "This report will save Etheria",
                    "name": "Sword of Protection report",
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
    mockito.when(request_handler).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find_by_id("reports", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    result = reports.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("report_id", ""),
                ("name", "Queen of Bright Moon report"),
                ("description", ""),
                ("notebook", ""),
                ("config", ""),
                ("created_by", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:08.371563",
                        "created_by": "glimmer@example.com",
                        "notebook": {
                            "metadata": {
                                "language_info": {
                                    "codemirror_mode": {
                                        "name": "ipython",
                                        "version": 3,
                                    },
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
                        "description": "This report leads the Rebellion",
                        "name": "Queen of Bright Moon report",
                        "report_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("report_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("name", ""),
                ("description", ""),
                ("notebook", ""),
                ("config", ""),
                ("created_by", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
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
    mockito.when(request_handler).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find("reports", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find(find_data):
    result = reports.find(
        find_data["params"][0][1],
        find_data["params"][1][1],
        find_data["params"][2][1],
        find_data["params"][3][1],
        find_data["params"][4][1],
        find_data["params"][5][1],
        find_data["params"][6][1],
        find_data["params"][7][1],
        find_data["params"][8][1],
        find_data["params"][9][1],
        find_data["params"][10][1],
    )
    assert result == find_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("name", "Horde Emperor report"),
                ("description", "This report rules the known universe"),
                (
                    "notebook",
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
                ),
                ("config", {"cpu": 2}),
                ("created_by", "hordeprime@example.com"),
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "hordeprime@example.com",
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
                    "description": "This report rules the known universe",
                    "name": "Horde Emperor report",
                    "report_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("name", "Horde Emperor report"),
                ("description", "This report rules the known universe"),
                (
                    "notebook",
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
                ),
                ("config", {"cpu": 2}),
                ("created_by", "hordeprime@example.com"),
            ],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new report",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).create(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).create("reports", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_create(create_data):
    result = reports.create(
        create_data["params"][0][1],
        create_data["params"][1][1],
        create_data["params"][2][1],
        create_data["params"][3][1],
        create_data["params"][4][1],
    )
    assert result == create_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Catra report"),
                (
                    "description",
                    "This report is trying to learn to process anger better",
                ),
                (
                    "notebook",
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
                ),
                ("config", {"cpu": 2}),
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "catra@example.com",
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
                    "description": "This report is trying to learn to process anger better",
                    "name": "Catra report",
                    "report_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Angella report"),
                ("description", ""),
                (
                    "notebook",
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
                ),
                ("config", {"cpu": 2}),
            ],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to update new report",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).update(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).update(
        "reports", request.param["id"], request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    result = reports.update(
        update_data["id"],
        update_data["params"][0][1],
        update_data["params"][1][1],
        update_data["params"][2][1],
        update_data["params"][3][1],
    )
    assert result == update_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).delete(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).delete("reports", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    result = reports.delete(delete_data["id"])
    assert result == delete_data["return"]
