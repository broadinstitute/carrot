import json

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import software


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
            "args": ["software", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "repository_url": "example.com/repo.git",
                    "description": "This software will save Etheria",
                    "name": "Sword of Protection software",
                    "software_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["software", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "title": "No software found",
                    "status": 404,
                    "detail": "No software found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(software).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(software).find_by_id(request.param["args"][2]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    runner = CliRunner()
    test_software = runner.invoke(carrot, find_by_id_data["args"])
    assert test_software.output == find_by_id_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "software",
                "find",
                "--software_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Sword of Protection software",
                "--description",
                "This software will save Etheria",
                "--repository_url",
                "example.com/repo.git",
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
                "Sword of Protection software",
                "This software will save Etheria",
                "example.com/repo.git",
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
                    "repository_url": "example.com/repo.git",
                    "description": "This software will save Etheria",
                    "name": "Sword of Protection software",
                    "software_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "software",
                "find",
                "--software_id",
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
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No software found",
                    "status": 404,
                    "detail": "No software found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(software).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(software).find(
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
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    runner = CliRunner()
    test_software = runner.invoke(carrot, find_data["args"])
    assert test_software.output == find_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "software",
                "create",
                "--name",
                "Sword of Protection software",
                "--description",
                "This software will save Etheria",
                "--repository_url",
                "example.com/repo.git",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection software",
                "This software will save Etheria",
                "example.com/repo.git",
                "adora@example.com",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "repository_url": "example.com/repo.git",
                    "description": "This software will save Etheria",
                    "name": "Sword of Protection software",
                    "software_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "software",
                "create",
                "--name",
                "Sword of Protection software",
                "--description",
                "This software will save Etheria",
                "--repository_url",
                "example.com/repo.git",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": ["software", "create"],
            "params": [],
            "return": "Usage: carrot_cli software create [OPTIONS]\n"
            "Try 'carrot_cli software create -h' for help.\n"
            "\n"
            "Error: Missing option '--name'.",
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(software).create(...).thenReturn(None)
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(software).create(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
        ).thenReturn(request.param["return"])
    return request.param


def test_create(create_data, caplog):
    runner = CliRunner()
    test_software = runner.invoke(carrot, create_data["args"])
    if "logging" in create_data:
        assert create_data["logging"] in caplog.text
    else:
        assert test_software.output == create_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "software",
                "update",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--description",
                "This new software replaced the broken one",
                "--name",
                "New Sword of Protection software",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection software",
                "This new software replaced the broken one",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "repository_url": "example.com/repo.git",
                    "description": "This new software replaced the broken one",
                    "name": "New Sword of Protection software",
                    "software_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "software",
                "update",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--description",
                "This new software replaced the broken one",
                "--name",
                "New Sword of Protection software",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection software",
                "This new software replaced the broken one",
            ],
            "from_name": {
                "name": "New Sword of Protection software",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "repository_url": "example.com/repo.git",
                            "description": "This new software replaced the broken one",
                            "name": "New Sword of Protection software",
                            "software_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "repository_url": "example.com/repo.git",
                    "description": "This new software replaced the broken one",
                    "name": "New Sword of Protection software",
                    "software_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["software", "update"],
            "params": [],
            "return": "Usage: carrot_cli software update [OPTIONS] SOFTWARE\n"
            "Try 'carrot_cli software update -h' for help.\n"
            "\n"
            "Error: Missing argument 'SOFTWARE'.",
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(software).update(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(software).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(software).update(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
        ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    runner = CliRunner()
    test_software = runner.invoke(carrot, update_data["args"])
    assert test_software.output == update_data["return"] + "\n"
