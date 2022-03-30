import json
import logging

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import runs, templates, tests


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
            "args": ["test", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test will save Etheria",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "name": "Sword of Protection test",
                    "template_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["test", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "title": "No test found",
                    "status": 404,
                    "detail": "No test found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(tests).find_by_id(request.param["args"][2]).thenReturn(
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
                "test",
                "find",
                "--test_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--template_id",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--name",
                "Sword of Protection test",
                "--description",
                "This test will save Etheria",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
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
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "Sword of Protection test",
                "This test will save Etheria",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "adora@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "created_by": "adora@example.com",
                        "description": "This test will save Etheria",
                        "test_input_defaults": {"in_greeted": "Cool Person"},
                        "test_option_defaults": {"option": "other_value"},
                        "eval_input_defaults": {
                            "in_output_filename": "test_greeting.txt"
                        },
                        "eval_option_defaults": {"option": "value"},
                        "name": "Sword of Protection test",
                        "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "find",
                "--test_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--template",
                "Sword of Protection template",
                "--name",
                "Sword of Protection test",
                "--description",
                "This test will save Etheria",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
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
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "Sword of Protection test",
                "This test will save Etheria",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "adora@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
            ],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template will save Etheria",
                            "test_wdl": "example.com/rebellion_test.wdl",
                            "eval_wdl": "example.com/rebellion_eval.wdl",
                            "name": "Sword of Protection template",
                            "pipeline_id": "123bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "created_by": "adora@example.com",
                        "description": "This test will save Etheria",
                        "test_input_defaults": {"in_greeted": "Cool Person"},
                        "test_option_defaults": {"option": "other_value"},
                        "eval_input_defaults": {
                            "in_output_filename": "test_greeting.txt"
                        },
                        "eval_option_defaults": {"option": "value"},
                        "name": "Sword of Protection test",
                        "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "find",
                "--test_id",
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
                "",
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No tests found",
                    "status": 404,
                    "detail": "No tests found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).find(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(tests).find(
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
        request.param["params"][13],
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
                "test",
                "create",
                "--template_id",
                "550e8400-e29b-41d4-a716-446655440000",
                "--name",
                "Sword of Protection test",
                "--description",
                "This test will save Etheria",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection test",
                "550e8400-e29b-41d4-a716-446655440000",
                "This test will save Etheria",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "adora@example.com",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test will save Etheria",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "test_option_defaults": {"option": "other_value"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "eval_option_defaults": {"option": "value"},
                    "name": "Sword of Protection test",
                    "template_id": "550e8400-e29b-41d4-a716-446655440000",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "create",
                "--template",
                "Sword of Protection template",
                "--name",
                "Sword of Protection test",
                "--description",
                "This test will save Etheria",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection test",
                "550e8400-e29b-41d4-a716-446655440000",
                "This test will save Etheria",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "adora@example.com",
            ],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template will save Etheria",
                            "test_wdl": "example.com/rebellion_test.wdl",
                            "eval_wdl": "example.com/rebellion_eval.wdl",
                            "name": "Sword of Protection template",
                            "pipeline_id": "123bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "550e8400-e29b-41d4-a716-446655440000",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test will save Etheria",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "test_option_defaults": {"option": "other_value"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "eval_option_defaults": {"option": "value"},
                    "name": "Sword of Protection test",
                    "template_id": "550e8400-e29b-41d4-a716-446655440000",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "create",
                "--template_id",
                "550e8400-e29b-41d4-a716-446655440000",
                "--name",
                "Sword of Protection test",
                "--description",
                "This test will save Etheria",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": ["test", "create"],
            "params": [],
            "return": "Usage: carrot_cli test create [OPTIONS]\n"
            "Try 'carrot_cli test create -h' for help.\n"
            "\n"
            "Error: Missing option '--name'.",
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).create(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(tests).create(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
            request.param["params"][4],
            request.param["params"][5],
            request.param["params"][6],
            request.param["params"][7],
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
                "test",
                "update",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--description",
                "This new test replaced the broken one",
                "--name",
                "New Sword of Protection test",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection test",
                "This new test replaced the broken one",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "test_option_defaults": {"option": "other_value"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "eval_option_defaults": {"option": "value"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "update",
                "Sword of Protection test",
                "--description",
                "This new test replaced the broken one",
                "--name",
                "New Sword of Protection test",
                "--test_input_defaults",
                "tests/data/mock_test_input.json",
                "--test_option_defaults",
                "tests/data/mock_test_options.json",
                "--eval_input_defaults",
                "tests/data/mock_eval_input.json",
                "--eval_option_defaults",
                "tests/data/mock_eval_options.json",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection test",
                "This new test replaced the broken one",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
            ],
            "from_name": {
                "name": "Sword of Protection test",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "",
                            "test_input_defaults": {"in_greeted": "Cool Person"},
                            "test_option_defaults": {"option": "other_value"},
                            "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                            "eval_option_defaults": {"option": "value"},
                            "name": "Sword of Protection test",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "test_option_defaults": {"option": "other_value"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "eval_option_defaults": {"option": "value"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["test", "update"],
            "params": [],
            "return": "Usage: carrot_cli test update [OPTIONS] TEST\n"
            "Try 'carrot_cli test update -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEST'.",
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).update(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(tests).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(tests).update(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
            request.param["params"][4],
            request.param["params"][5],
            request.param["params"][6],
        ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    runner = CliRunner()
    result = runner.invoke(carrot, update_data["args"])
    assert result.output == update_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": ["test", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["test", "delete", "New Sword of Protection test"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "from_name": {
                "name": "New Sword of Protection test",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This test replaced the broken one",
                            "test_input_defaults": {"in_greeted": "Cool Person"},
                            "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                            "name": "New Sword of Protection test",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "email": "adora@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": ["test", "delete", "-y", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["test", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "message": "Test with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. Are "
                "you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": ["test", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This test replaced the broken one",
                    "test_input_defaults": {"in_greeted": "Cool Person"},
                    "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                    "name": "New Sword of Protection test",
                    "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Test with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. Are "
                "you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["test", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "title": "No test found",
                    "status": 404,
                    "detail": "No test found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {
                    "title": "No test found",
                    "status": 404,
                    "detail": "No test found with the specified ID",
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
    mockito.when(tests).delete(...).thenReturn(None)
    mockito.when(tests).find_by_id(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(tests).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(tests).delete(request.param["id"]).thenReturn(request.param["return"])
    mockito.when(tests).find_by_id(request.param["id"]).thenReturn(
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
                "test",
                "run",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Queen of Bright Moon run",
                "--test_input",
                "tests/data/mock_test_input.json",
                "--test_options",
                "tests/data/mock_test_options.json",
                "--eval_input",
                "tests/data/mock_eval_input.json",
                "--eval_options",
                "tests/data/mock_eval_options.json",
                "--created_by",
                "glimmer@example.com",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "glimmer@example.com",
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_mother": "Angella"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_friend": "Bow"},
                        "eval_options": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "name": "Queen of Bright Moon run",
                        "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "run",
                "New Sword of Protection test",
                "--name",
                "Queen of Bright Moon run",
                "--test_input",
                "tests/data/mock_test_input.json",
                "--test_options",
                "tests/data/mock_test_options.json",
                "--eval_input",
                "tests/data/mock_eval_input.json",
                "--eval_options",
                "tests/data/mock_eval_options.json",
                "--created_by",
                "glimmer@example.com",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "glimmer@example.com",
            ],
            "from_name": {
                "name": "New Sword of Protection test",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This test replaced the broken one",
                            "test_input_defaults": {"in_greeted": "Cool Person"},
                            "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                            "name": "New Sword of Protection test",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_mother": "Angella"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_friend": "Bow"},
                        "eval_options": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "name": "Queen of Bright Moon run",
                        "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "run",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                "--created_by",
                "frosta@example.com",
            ],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                "",
                "",
                "",
                "",
                "",
                "frosta@example.com",
            ],
            "return": json.dumps(
                {
                    "detail": "Error while attempting to query the database: NotFound",
                    "status": 500,
                    "title": "Server error",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "run",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Queen of Bright Moon run",
                "--test_input",
                "tests/data/mock_test_input.json",
                "--test_options",
                "tests/data/mock_test_options.json",
                "--eval_input",
                "tests/data/mock_eval_input.json",
                "--eval_options",
                "tests/data/mock_eval_options.json",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created by, "
            "there must be a value set for email.",
        },
        {
            "args": [
                "test",
                "run",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Queen of Bright Moon run",
                "--test_input",
                "nonexistent_file.json",
                "--eval_input",
                "tests/data/mock_eval_input.json",
                "--created_by",
                "glimmer@example.com",
            ],
            "params": [],
            "logging": "Encountered FileNotFound error when trying to read nonexistent_file.json",
        },
    ]
)
def run_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).run(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(tests).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    if len(request.param["params"]) > 0:
        mockito.when(tests).run(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
            request.param["params"][4],
            request.param["params"][5],
            request.param["params"][6],
        ).thenReturn(request.param["return"])
    return request.param


def test_run(run_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, run_data["args"])
    if "logging" in run_data:
        assert run_data["logging"] in caplog.text
    else:
        assert result.output == run_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "test",
                "find_runs",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--name",
                "Queen of Bright Moon run",
                "--status",
                "succeeded",
                "--test_input",
                "tests/data/mock_test_input.json",
                "--test_options",
                "tests/data/mock_test_options.json",
                "--eval_input",
                "tests/data/mock_eval_input.json",
                "--eval_options",
                "tests/data/mock_eval_options.json",
                "--test_cromwell_job_id",
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "--eval_cromwell_job_id",
                "03958293-6b71-429c-a4de-8e90222488cd",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--created_by",
                "glimmer@example.com",
                "--finished_before",
                "2020-10-00T00:00:00.000000",
                "--finished_after",
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
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_mother": "Angella"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_friend": "Bow"},
                        "eval_options": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "eval_cromwell_job_id": "03958293-6b71-429c-a4de-8e90222488cd",
                        "name": "Queen of Bright Moon run",
                        "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "find_runs",
                "New Sword of Protection test",
                "--name",
                "Queen of Bright Moon run",
                "--status",
                "succeeded",
                "--test_input",
                "tests/data/mock_test_input.json",
                "--test_options",
                "tests/data/mock_test_options.json",
                "--eval_input",
                "tests/data/mock_eval_input.json",
                "--eval_options",
                "tests/data/mock_eval_options.json",
                "--test_cromwell_job_id",
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "--eval_cromwell_job_id",
                "03958293-6b71-429c-a4de-8e90222488cd",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--created_by",
                "glimmer@example.com",
                "--finished_before",
                "2020-10-00T00:00:00.000000",
                "--finished_after",
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
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
            ],
            "from_name": {
                "name": "New Sword of Protection test",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This test replaced the broken one",
                            "test_input_defaults": {"in_greeted": "Cool Person"},
                            "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                            "name": "New Sword of Protection test",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_mother": "Angella"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_friend": "Bow"},
                        "eval_options": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "eval_cromwell_job_id": "03958293-6b71-429c-a4de-8e90222488cd",
                        "name": "Queen of Bright Moon run",
                        "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["test", "find_runs", "986325ba-06fe-4b1a-9e96-47d4f36bf819"],
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
                "",
                "",
                "",
                "",
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No run found",
                    "status": 404,
                    "detail": "No runs found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "find_runs",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                "--test_input",
                "nonexistent_file.json",
            ],
            "params": [],
            "logging": "Encountered FileNotFound error when trying to read nonexistent_file.json",
        },
    ]
)
def find_runs_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(runs).find(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(tests).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    if len(request.param["params"]) > 0:
        mockito.when(runs).find(
            "tests",
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
            request.param["params"][13],
            request.param["params"][14],
            request.param["params"][15],
            request.param["params"][16],
        ).thenReturn(request.param["return"])
    return request.param


def test_find_runs(find_runs_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, find_runs_data["args"])
    if "logging" in find_runs_data:
        assert find_runs_data["logging"] in caplog.text
    else:
        assert result.output == find_runs_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "test",
                "subscribe",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "test",
                    "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "email": "netossa@example.com",
                    "created_at": "2020-09-23T19:41:46.839880",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "subscribe",
                "New Sword of Protection test",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "from_name": {
                "name": "New Sword of Protection test",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This test replaced the broken one",
                            "test_input_defaults": {"in_greeted": "Cool Person"},
                            "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                            "name": "New Sword of Protection test",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "test",
                    "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "email": "netossa@example.com",
                    "created_at": "2020-09-23T19:41:46.839880",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "test",
                "subscribe",
                "89657859-06fe-4b1a-9e96-47d4f36bf819",
                "--email",
                "spinnerella@example.com",
            ],
            "params": [
                "89657859-06fe-4b1a-9e96-47d4f36bf819",
                "spinnerella@example.com",
            ],
            "return": json.dumps(
                {
                    "title": "No test found",
                    "status": 404,
                    "detail": "No test found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["test", "subscribe", "89657859-06fe-4b1a-9e96-47d4f36bf819"],
            "params": ["89657859-06fe-4b1a-9e96-47d4f36bf819", "frosta@example.com"],
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "test",
                    "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "email": "frosta@example.com",
                    "created_at": "2020-09-23T19:41:46.839880",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def subscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).subscribe(...).thenReturn(None)
    mockito.when(config).load_var_no_error("email").thenReturn("frosta@example.com")
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(tests).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(tests).subscribe(
        request.param["params"][0], request.param["params"][1]
    ).thenReturn(request.param["return"])
    return request.param


def test_subscribe(subscribe_data):
    runner = CliRunner()
    result = runner.invoke(carrot, subscribe_data["args"])
    assert result.output == subscribe_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "test",
                "unsubscribe",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "test",
                "unsubscribe",
                "New Sword of Protection test",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "from_name": {
                "name": "New Sword of Protection test",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This test replaced the broken one",
                            "test_input_defaults": {"in_greeted": "Cool Person"},
                            "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                            "name": "New Sword of Protection test",
                            "template_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "test_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "test",
                "unsubscribe",
                "89657859-06fe-4b1a-9e96-47d4f36bf819",
                "--email",
                "spinnerella@example.com",
            ],
            "params": [
                "89657859-06fe-4b1a-9e96-47d4f36bf819",
                "spinnerella@example.com",
            ],
            "return": json.dumps(
                {
                    "title": "No subscription found",
                    "status": 404,
                    "detail": "No subscription found for the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["test", "unsubscribe", "89657859-06fe-4b1a-9e96-47d4f36bf819"],
            "params": ["89657859-06fe-4b1a-9e96-47d4f36bf819", "frosta@example.com"],
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
    ]
)
def unsubscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(tests).unsubscribe(...).thenReturn(None)
    mockito.when(config).load_var_no_error("email").thenReturn("frosta@example.com")
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(tests).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(tests).unsubscribe(
        request.param["params"][0], request.param["params"][1]
    ).thenReturn(request.param["return"])
    return request.param


def test_unsubscribe(unsubscribe_data):
    runner = CliRunner()
    result = runner.invoke(carrot, unsubscribe_data["args"])
    assert result.output == unsubscribe_data["return"] + "\n"
