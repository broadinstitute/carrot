import json

from click.testing import CliRunner

import logging
import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import results, template_results, templates


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
            "args": ["result", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "file",
                    "description": "This result will save Etheria",
                    "name": "Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["result", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "title": "No result found",
                    "status": 404,
                    "detail": "No result found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(results).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(results).find_by_id(request.param["args"][2]).thenReturn(
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
                "result",
                "find",
                "--result_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Sword of Protection result",
                "--description",
                "This result will save Etheria",
                "--result_type",
                "numeric",
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
                "Sword of Protection result",
                "This result will save Etheria",
                "numeric",
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
                    "result_type": "numeric",
                    "description": "This result will save Etheria",
                    "name": "Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "result",
                "find",
                "--result_id",
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
                    "title": "No results found",
                    "status": 404,
                    "detail": "No results found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(results).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(results).find(
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
    result = runner.invoke(carrot, find_data["args"])
    assert result.output == find_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "result",
                "create",
                "--name",
                "Sword of Protection result",
                "--description",
                "This result will save Etheria",
                "--result_type",
                "numeric",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection result",
                "This result will save Etheria",
                "numeric",
                "adora@example.com",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "numeric",
                    "description": "This result will save Etheria",
                    "name": "Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "result",
                "create",
                "--name",
                "Sword of Protection result",
                "--description",
                "This result will save Etheria",
                "--result_type",
                "numeric",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": ["result", "create"],
            "params": [],
            "return": "Usage: carrot_cli result create [OPTIONS]\n"
            "Try 'carrot_cli result create -h' for help.\n"
            "\n"
            "Error: Missing option '--name'.",
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(results).create(...).thenReturn(None)
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(results).create(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
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
                "result",
                "update",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--description",
                "This new result replaced the broken one",
                "--name",
                "New Sword of Protection result",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection result",
                "This new result replaced the broken one",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "file",
                    "description": "This new result replaced the broken one",
                    "name": "New Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "result",
                "update",
                "Old Sword of Protection result",
                "--description",
                "This new result replaced the broken one",
                "--name",
                "New Sword of Protection result",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection result",
                "This new result replaced the broken one",
            ],
            "from_name": {
                "name": "Old Sword of Protection result",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "result_type": "file",
                            "description": "This old result is old",
                            "name": "Old Sword of Protection result",
                            "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                    "result_type": "file",
                    "description": "This new result replaced the broken one",
                    "name": "New Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["result", "update"],
            "params": [],
            "return": "Usage: carrot_cli result update [OPTIONS] RESULT\n"
            "Try 'carrot_cli result update -h' for help.\n"
            "\n"
            "Error: Missing argument 'RESULT'.",
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(results).update(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(results).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(results).update(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
        ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    runner = CliRunner()
    result = runner.invoke(carrot, update_data["args"])
    assert result.output == update_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": ["result", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "file",
                    "description": "This new result replaced the broken one",
                    "name": "New Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["result", "delete", "-y", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "file",
                    "description": "This new result replaced the broken one",
                    "name": "New Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["result", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "file",
                    "description": "This new result replaced the broken one",
                    "name": "New Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "message": "Result with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": ["result", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "result_type": "file",
                    "description": "This new result replaced the broken one",
                    "name": "New Sword of Protection result",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Result with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["result", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "title": "No result found",
                    "status": 404,
                    "detail": "No result found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {
                    "title": "No result found",
                    "status": 404,
                    "detail": "No result found with the specified ID",
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
    mockito.when(results).delete(...).thenReturn(None)
    mockito.when(results).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(results).delete(request.param["id"]).thenReturn(
        request.param["return"]
    )
    mockito.when(results).find_by_id(request.param["id"]).thenReturn(
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
                "result",
                "map_to_template",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "out_horde_tanks",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "out_horde_tanks",
                "adora@example.com",
            ],
            "return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "out_horde_tanks",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "result",
                "map_to_template",
                "Horde Tanks",
                "Horde Template",
                "out_horde_tanks",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "out_horde_tanks",
                "adora@example.com",
            ],
            "from_names": {
                "result_name": "Horde Tanks",
                "result_return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "result_type": "numeric",
                            "description": "How many tanks",
                            "name": "Horde Tanks",
                            "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
                "template_name": "Horde Template",
                "template_return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template is for horde stuff",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "name": "Horde Template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "out_horde_tanks",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "result",
                "map_to_template",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "out_horde_tanks",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": ["result", "map_to_template"],
            "params": [],
            "return": "Usage: carrot_cli result map_to_template [OPTIONS] RESULT TEMPLATE RESULT_KEY\n"
            "Try 'carrot_cli result map_to_template -h' for help.\n"
            "\n"
            "Error: Missing argument 'RESULT'.",
        },
    ]
)
def map_to_template_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_results).create_map(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(results).find(
            name=request.param["from_names"]["result_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["result_return"])
        mockito.when(templates).find(
            name=request.param["from_names"]["template_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["template_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(template_results).create_map(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
        ).thenReturn(request.param["return"])
    return request.param


def test_map_to_template(map_to_template_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, map_to_template_data["args"])
    if "logging" in map_to_template_data:
        assert map_to_template_data["logging"] in caplog.text
    else:
        assert result.output == map_to_template_data["return"] + "\n"
