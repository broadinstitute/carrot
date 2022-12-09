import json
import logging

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import pipelines, report_maps, reports, results, runs, template_reports, template_results, templates


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
            "args": ["template", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template will save Etheria",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "name": "Sword of Protection template",
                    "pipeline_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["template", "find_by_id", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "return": json.dumps(
                {
                    "title": "No template found",
                    "status": 404,
                    "detail": "No template found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(templates).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(templates).find_by_id(request.param["args"][2]).thenReturn(
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
                "template",
                "find",
                "--template_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--pipeline_id",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--name",
                "Sword of Protection template",
                "--description",
                "This template will save Etheria",
                "--test_wdl",
                "example.com/rebellion_test.wdl",
                "--eval_wdl",
                "example.com/rebellion_eval.wdl",
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
                "Sword of Protection template",
                "This template will save Etheria",
                "example.com/rebellion_test.wdl",
                "example.com/rebellion_eval.wdl",
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
                        "description": "This template will save Etheria",
                        "test_wdl": "example.com/rebellion_test.wdl",
                        "eval_wdl": "example.com/rebellion_eval.wdl",
                        "name": "Sword of Protection template",
                        "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find",
                "--template_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--pipeline",
                "Sword of Protection pipeline",
                "--name",
                "Sword of Protection template",
                "--description",
                "This template will save Etheria",
                "--test_wdl",
                "example.com/rebellion_test.wdl",
                "--eval_wdl",
                "example.com/rebellion_eval.wdl",
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
                "Sword of Protection template",
                "This template will save Etheria",
                "example.com/rebellion_test.wdl",
                "example.com/rebellion_eval.wdl",
                "adora@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
            ],
            "from_name": {
                "name": "Sword of Protection pipeline",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This is the old description for this pipeline",
                            "name": "Sword of Protection pipeline",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                        "description": "This template will save Etheria",
                        "test_wdl": "example.com/rebellion_test.wdl",
                        "eval_wdl": "example.com/rebellion_eval.wdl",
                        "name": "Sword of Protection template",
                        "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find",
                "--template_id",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No templates found",
                    "status": 404,
                    "detail": "No templates found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(templates).find(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(pipelines).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(templates).find(
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
                "template",
                "create",
                "--pipeline_id",
                "550e8400-e29b-41d4-a716-446655440000",
                "--name",
                "Sword of Protection template",
                "--description",
                "This template will save Etheria",
                "--test_wdl",
                "example.com/she-ra_test.wdl",
                "--test_wdl_dependencies",
                "example.com/she-ra_test_dep.zip",
                "--eval_wdl",
                "example.com/she-ra_eval.wdl",
                "--eval_wdl_dependencies",
                "example.com/she-ra_eval_dep.zip",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection template",
                "550e8400-e29b-41d4-a716-446655440000",
                "This template will save Etheria",
                "example.com/she-ra_test.wdl",
                "example.com/she-ra_test_dep.zip",
                "example.com/she-ra_eval.wdl",
                "example.com/she-ra_eval_dep.zip",
                "adora@example.com",
                None
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template will save Etheria",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "test_wdl_dependencies": "example.com/she-ra_test_dep.zip",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                    "name": "Sword of Protection template",
                    "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "create",
                "--pipeline",
                "Sword of Protection pipeline",
                "--name",
                "Sword of Protection template",
                "--description",
                "This template will save Etheria",
                "--test_wdl",
                "example.com/she-ra_test.wdl",
                "--test_wdl_dependencies",
                "example.com/she-ra_test_dep.zip",
                "--eval_wdl",
                "example.com/she-ra_eval.wdl",
                "--eval_wdl_dependencies",
                "example.com/she-ra_eval_dep.zip",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "Sword of Protection template",
                "550e8400-e29b-41d4-a716-446655440000",
                "This template will save Etheria",
                "example.com/she-ra_test.wdl",
                "example.com/she-ra_test_dep.zip",
                "example.com/she-ra_eval.wdl",
                "example.com/she-ra_eval_dep.zip",
                "adora@example.com",
                None
            ],
            "from_name": {
                "name": "Sword of Protection pipeline",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This is the old description for this pipeline",
                            "name": "Sword of Protection pipeline",
                            "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
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
                    "description": "This template will save Etheria",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "test_wdl_dependencies": "example.com/she-ra_test_dep.zip",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                    "name": "Sword of Protection template",
                    "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "create",
                "--name",
                "Sword of Protection template copy",
                "--description",
                "This template will save Etheria again",
                "--test_wdl",
                "example.com/she-ra_test2.wdl",
                "--test_wdl_dependencies",
                "example.com/she-ra_test_dep2.zip",
                "--created_by",
                "adora2@example.com",
                "--copy",
                "Sword of Protection template"
            ],
            "params": [
                "Sword of Protection template copy",
                None,
                "This template will save Etheria again",
                "example.com/she-ra_test2.wdl",
                "example.com/she-ra_test_dep2.zip",
                None,
                None,
                "adora2@example.com",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819"
            ],
            "copy": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template will save Etheria",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "test_wdl_dependencies": "example.com/she-ra_test_dep.zip",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                            "name": "Sword of Protection template",
                            "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "created_at": "2020-09-18T18:48:06.371563",
                    "created_by": "adora2@example.com",
                    "description": "This template will save Etheria again",
                    "test_wdl": "example.com/she-ra_test2.wdl",
                    "test_wdl_dependencies": "example.com/she-ra_test_dep2.zip",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                    "name": "Sword of Protection template copy",
                    "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                    "template_id": "abc87859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "create",
                "--pipeline_id",
                "550e8400-e29b-41d4-a716-446655440000",
                "--name",
                "Sword of Protection template",
                "--description",
                "This template will save Etheria",
                "--test_wdl",
                "example.com/she-ra_test.wdl",
                "--test_wdl_dependencies",
                "example.com/she-ra_test_dep.zip",
                "--eval_wdl",
                "example.com/she-ra_eval.wdl",
                "--eval_wdl_dependencies",
                "example.com/she-ra_eval_dep.zip",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created_by, "
            "there must be a value set for email.",
        },
        {
            "args": ["template", "create"],
            "params": [],
            "logging": "If a value is not specified for '--copy', then '--name', '--pipeline', '--test_wdl', and "
                       "'--eval_wdl are required."
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(templates).create(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(pipelines).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    if "copy" in request.param:
        mockito.when(templates).find(
            name=request.param["copy"]["name"],
            limit=2
        ).thenReturn(request.param["copy"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(templates).create(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3],
            request.param["params"][4],
            request.param["params"][5],
            request.param["params"][6],
            request.param["params"][7],
            request.param["params"][8],
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
                "template",
                "update",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--description",
                "This new template replaced the broken one",
                "--name",
                "New Sword of Protection template",
                "--test_wdl",
                "example.com/she-ra_test.wdl",
                "--test_wdl_dependencies",
                "example.com/she-ra_test_dep.zip",
                "--eval_wdl",
                "example.com/she-ra_eval.wdl",
                "--eval_wdl_dependencies",
                "example.com/she-ra_eval_dep.zip",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection template",
                "This new template replaced the broken one",
                "example.com/she-ra_test.wdl",
                "example.com/she-ra_test_dep.zip",
                "example.com/she-ra_eval.wdl",
                "example.com/she-ra_eval_dep.zip",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "test_wdl_dependencies": "example.com/she-ra_test_dep.zip",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "update",
                "Sword of Protection template",
                "--description",
                "This new template replaced the broken one",
                "--name",
                "New Sword of Protection template",
                "--test_wdl",
                "example.com/she-ra_test.wdl",
                "--test_wdl_dependencies",
                "example.com/she-ra_test_dep.zip",
                "--eval_wdl",
                "example.com/she-ra_eval.wdl",
                "--eval_wdl_dependencies",
                "example.com/she-ra_eval_dep.zip",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "New Sword of Protection template",
                "This new template replaced the broken one",
                "example.com/she-ra_test.wdl",
                "example.com/she-ra_test_dep.zip",
                "example.com/she-ra_eval.wdl",
                "example.com/she-ra_eval_dep.zip",
            ],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "test_wdl_dependencies": "example.com/she-ra_test_dep.zip",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                            "name": "Sword of Protection template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "test_wdl_dependencies": "example.com/she-ra_test_dep.zip",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "eval_wdl_dependencies": "example.com/she-ra_eval_dep.zip",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["template", "update"],
            "params": [],
            "return": "Usage: carrot_cli template update [OPTIONS] TEMPLATE\n"
            "Try 'carrot_cli template update -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(templates).update(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(templates).update(
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
            "args": ["template", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["template", "delete", "New Sword of Protection template"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "from_name": {
                "name": "New Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template replaced the broken one",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "name": "New Sword of Protection template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "template",
                "delete",
                "-y",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["template", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "message": "Template with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": ["template", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "description": "This template replaced the broken one",
                    "test_wdl": "example.com/she-ra_test.wdl",
                    "eval_wdl": "example.com/she-ra_eval.wdl",
                    "name": "New Sword of Protection template",
                    "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Template with id cd987859-06fe-4b1a-9e96-47d4f36bf819 was created by adora@example.com. "
                "Are you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["template", "delete", "cd987859-06fe-4b1a-9e96-47d4f36bf819"],
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "find_return": json.dumps(
                {
                    "title": "No template found",
                    "status": 404,
                    "detail": "No template found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {
                    "title": "No template found",
                    "status": 404,
                    "detail": "No template found with the specified ID",
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
    mockito.when(templates).delete(...).thenReturn(None)
    mockito.when(templates).find_by_id(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(templates).delete(request.param["id"]).thenReturn(
        request.param["return"]
    )
    mockito.when(templates).find_by_id(request.param["id"]).thenReturn(
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
                "template",
                "find_runs",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "--software_name",
                "test_software",
                "--commit_or_tag",
                "1.1.0",
                "--commit_or_tag",
                "1.1.1",
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
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                {"name": "test_software", "commits_and_tags": ["1.1.0", "1.1.1"]},
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
                None
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_greeted": "Cool Person", "docker": "image_build:test_software|1.1.0"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_output_filename": "test_greeting.txt"},
                        "eval_options": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "run_group_id": "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "template",
                "find_runs",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "--software_name",
                "test_software",
                "--commit_count",
                1,
                "--software_branch",
                "master",
                "--tags_only",
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
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                {"name": "test_software", "count": 1, "branch": "master", "tags_only": True},
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
                None
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_greeted": "Cool Person", "docker": "image_build:test_software|1.1.0"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_output_filename": "test_greeting.txt"},
                        "eval_options": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "run_group_id": "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "template",
                "find_runs",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "--zip_csv",
                "csvs.zip"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                None,
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
                "csvs.zip",
            ],
            "return": "Success!"
        },
        {
            "args": [
                "template",
                "find_runs",
                "Sword of Protection template",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                None,
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0,
                None
            ],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template replaced the broken one",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "name": "New Sword of Protection template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": "2020-09-16T18:58:06.371563",
                        "created_by": "glimmer@example.com",
                        "test_input_defaults": {"in_greeted": "Cool Person"},
                        "test_option_defaults": {"option": "other_value"},
                        "eval_input_defaults": {"in_output_filename": "test_greeting.txt"},
                        "eval_option_defaults": {"option": "value"},
                        "status": "succeeded",
                        "results": {},
                        "run_group_id": "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["template", "find_runs", "986325ba-06fe-4b1a-9e96-47d4f36bf819"],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                20,
                0,
                None,
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
                "template",
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
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    if len(request.param["params"]) > 0:
        mockito.when(runs).find(
            "templates",
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
            request.param["params"][17],
            request.param["params"][18],
            csv=request.param["params"][19],
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
                "template",
                "create_report_for_runs",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "--created_by",
                "adora@example.com",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "--software_name",
                "test_software",
                "--commit_or_tag",
                "1.1.0",
                "--commit_or_tag",
                "1.1.1",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--run_created_by",
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
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "adora@example.com",
                "templates",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                {"name": "test_software", "commits_and_tags": ["1.1.0", "1.1.1"]},
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0
            ],
            "return": json.dumps(
                {
                    "entity_id": "128abc85-06fe-4b1a-9e96-47d4f36bf819",
                    "entity_type": "run_group",
                    "report_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                    "status": "created",
                    "results": {},
                    "cromwell_job_id": "8f1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                "template",
                "create_report_for_runs",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "--created_by",
                "adora@example.com",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "--software_name",
                "test_software",
                "--commit_count",
                1,
                "--software_branch",
                "master",
                "--tags_only",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--run_created_by",
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
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "adora@example.com",
                "templates",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                {"name": "test_software", "count": 1, "branch": "master", "tags_only": True},
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0
            ],
            "return": json.dumps(
                {
                    "entity_id": "128abc85-06fe-4b1a-9e96-47d4f36bf819",
                    "entity_type": "run_group",
                    "report_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                    "status": "created",
                    "results": {},
                    "cromwell_job_id": "8f1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                "template",
                "create_report_for_runs",
                "Sword of Protection template",
                "Sword of Protection report",
                "--created_by",
                "adora@example.com",
                "--run_group_id",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
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
                "--run_created_by",
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
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "adora@example.com",
                "templates",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "ad487859-06fe-4b1a-9e96-47d4f36bf819",
                "Queen of Bright Moon run",
                "succeeded",
                {"in_greeted": "Cool Person"},
                {"option": "other_value"},
                {"in_output_filename": "test_greeting.txt"},
                {"option": "value"},
                "d9855002-6b71-429c-a4de-8e90222488cd",
                "03958293-6b71-429c-a4de-8e90222488cd",
                None,
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "glimmer@example.com",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(name)",
                1,
                0
            ],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template replaced the broken one",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "name": "New Sword of Protection template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "report_from_name": {
                "name": "Sword of Protection report",
                "return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This new report replaced the broken one",
                            "name": "Sword of Protection report",
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
                            "report_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                ),
            },
            "return": json.dumps(
                {
                    "entity_id": "128abc85-06fe-4b1a-9e96-47d4f36bf819",
                    "entity_type": "run_group",
                    "report_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                    "status": "created",
                    "results": {},
                    "cromwell_job_id": "8f1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                "template",
                "create_report_for_runs",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "--created_by",
                "adora@example.com"
            ],
            "params": [
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "adora@example.com",
                "templates",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                20,
                0
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
                "template",
                "create_report_for_runs",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
                "--created_by",
                "adora@example.com",
                "--test_input",
                "nonexistent_file.json",
            ],
            "params": [],
            "logging": "Encountered FileNotFound error when trying to read nonexistent_file.json",
        },
    ]
)
def create_report_for_runs_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(report_maps).create_map_from_run_query(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    if "report_from_name" in request.param:
        mockito.when(reports).find(
            name=request.param["report_from_name"]["name"],
            limit=2
        ).thenReturn(request.param["report_from_name"]["return"])
    # Mock up request response
    if len(request.param["params"]) > 0:
        mockito.when(report_maps).create_map_from_run_query(
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
            request.param["params"][17],
            request.param["params"][18],
            request.param["params"][19],
            request.param["params"][20],
            request.param["params"][21],
        ).thenReturn(request.param["return"])
    return request.param


def test_create_report_for_runs(create_report_for_runs_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, create_report_for_runs_data["args"])
    if "logging" in create_report_for_runs_data:
        assert create_report_for_runs_data["logging"] in caplog.text
    else:
        assert result.output == create_report_for_runs_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "subscribe",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "template",
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
                "template",
                "subscribe",
                "Sword of Protection template",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template replaced the broken one",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "name": "New Sword of Protection template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "template",
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
                "template",
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
                    "title": "No template found",
                    "status": 404,
                    "detail": "No template found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["template", "subscribe", "89657859-06fe-4b1a-9e96-47d4f36bf819"],
            "params": ["89657859-06fe-4b1a-9e96-47d4f36bf819", "frosta@example.com"],
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "template",
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
    mockito.when(templates).subscribe(...).thenReturn(None)
    mockito.when(config).load_var_no_error("email").thenReturn("frosta@example.com")
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(templates).subscribe(
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
                "template",
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
                "template",
                "unsubscribe",
                "Sword of Protection template",
                "--email",
                "netossa@example.com",
            ],
            "params": ["cd987859-06fe-4b1a-9e96-47d4f36bf819", "netossa@example.com"],
            "from_name": {
                "name": "Sword of Protection template",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This template replaced the broken one",
                            "test_wdl": "example.com/she-ra_test.wdl",
                            "eval_wdl": "example.com/she-ra_eval.wdl",
                            "name": "New Sword of Protection template",
                            "pipeline_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "template",
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
            "args": ["template", "unsubscribe", "89657859-06fe-4b1a-9e96-47d4f36bf819"],
            "params": ["89657859-06fe-4b1a-9e96-47d4f36bf819", "frosta@example.com"],
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
    ]
)
def unsubscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(templates).unsubscribe(...).thenReturn(None)
    mockito.when(config).load_var_no_error("email").thenReturn("frosta@example.com")
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(templates).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(templates).unsubscribe(
        request.param["params"][0], request.param["params"][1]
    ).thenReturn(request.param["return"])
    return request.param


def test_unsubscribe(unsubscribe_data):
    runner = CliRunner()
    result = runner.invoke(carrot, unsubscribe_data["args"])
    assert result.output == unsubscribe_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "map_to_result",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                "template",
                "map_to_result",
                "Horde Template",
                "Horde Tanks",
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
                            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "template",
                "map_to_result",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "out_horde_tanks",
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created_by, "
            "there must be a value set for email.",
        },
        {
            "args": ["template", "map_to_result"],
            "params": [],
            "return": "Usage: carrot_cli template map_to_result [OPTIONS] TEMPLATE RESULT RESULT_KEY\n"
            "Try 'carrot_cli template map_to_result -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def map_to_result_data(request):
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


def test_map_to_result(map_to_result_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, map_to_result_data["args"])
    if "logging" in map_to_result_data:
        assert map_to_result_data["logging"] in caplog.text
    else:
        assert result.output == map_to_result_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "find_result_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                "template",
                "find_result_map_by_id",
                "Horde Template",
                "Horde Tanks",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
            "args": ["template", "find_result_map_by_id"],
            "params": [],
            "return": "Usage: carrot_cli template find_result_map_by_id [OPTIONS] TEMPLATE RESULT\n"
            "Try 'carrot_cli template find_result_map_by_id -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def find_result_map_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_results).find_map_by_ids(...).thenReturn(None)
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
        mockito.when(template_results).find_map_by_ids(
            request.param["params"][0],
            request.param["params"][1],
        ).thenReturn(request.param["return"])
    return request.param


def test_find_result_map_by_id(find_result_map_by_id_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_result_map_by_id_data["args"])
    assert result.output == find_result_map_by_id_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "find_result_maps",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--result_id",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--result_key",
                "sword_of_protection_key",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(result_key)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "sword_of_protection_key",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "adora@example.com",
                "asc(result_key)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "result_key": "sword_of_protection_key",
                        "created_at": "2020-09-24T19:07:59.311462",
                        "created_by": "adora@example.com",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find_result_maps",
                "Horde Template",
                "--result",
                "Horde Tanks",
                "--result_key",
                "sword_of_protection_key",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(result_key)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "sword_of_protection_key",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "adora@example.com",
                "asc(result_key)",
                1,
                0,
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
                            "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                            "pipeline_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                [
                    {
                        "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "result_key": "sword_of_protection_key",
                        "created_at": "2020-09-24T19:07:59.311462",
                        "created_by": "adora@example.com",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find_result_maps",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                None,
                None,
                None,
                None,
                None,
                None,
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No template_results found",
                    "status": 404,
                    "detail": "No template_results found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_result_maps_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_results).find_maps(...).thenReturn(None)
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
    # Mock up request response
    mockito.when(template_results).find_maps(
        request.param["params"][0],
        request.param["params"][1],
        request.param["params"][2],
        request.param["params"][3],
        request.param["params"][4],
        request.param["params"][5],
        request.param["params"][6],
        request.param["params"][7],
        request.param["params"][8],
    ).thenReturn(request.param["return"])
    return request.param


def test_find_result_maps(find_result_maps_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_result_maps_data["args"])
    assert result.output == find_result_maps_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "delete_result_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "sword_of_protection_key",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
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
                "template",
                "delete_result_map_by_id",
                "Horde Template",
                "Horde Tanks",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "sword_of_protection_key",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
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
                            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "template",
                "delete_result_map_by_id",
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
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "sword_of_protection_key",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
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
                "template",
                "delete_result_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "sword_of_protection_key",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
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
                "message": "Mapping for template with id cd987859-06fe-4b1a-9e96-47d4f36bf819 and "
                "result with id 3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8 was created by adora@example.com. Are "
                "you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": [
                "template",
                "delete_result_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "result_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "result_key": "sword_of_protection_key",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Mapping for template with id cd987859-06fe-4b1a-9e96-47d4f36bf819 and "
                "result with id 3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8 was created by adora@example.com. Are "
                "you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": [
                "template",
                "delete_result_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "ids": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            ],
            "find_return": json.dumps(
                {
                    "title": "No template_result found",
                    "status": 404,
                    "detail": "No template_result found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "adora@example.com",
            "return": json.dumps(
                {
                    "title": "No template_result found",
                    "status": 404,
                    "detail": "No template_result found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["template", "delete_result_map_by_id"],
            "ids": [],
            "email": "adora@example.com",
            "return": "Usage: carrot_cli template delete_result_map_by_id [OPTIONS] TEMPLATE RESULT\n"
            "Try 'carrot_cli template delete_result_map_by_id -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def delete_result_map_by_id_data(request):
    # We want to load the value from "email" from config
    mockito.when(config).load_var("email").thenReturn(request.param["email"])
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_results).delete_map_by_ids(...).thenReturn(None)
    mockito.when(template_results).find_map_by_ids(...).thenReturn(None)
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
    if len(request.param["ids"]) > 0:
        mockito.when(template_results).delete_map_by_ids(
            request.param["ids"][0],
            request.param["ids"][1],
        ).thenReturn(request.param["return"])
        mockito.when(template_results).find_map_by_ids(
            request.param["ids"][0],
            request.param["ids"][1],
        ).thenReturn(request.param["find_return"])
    return request.param


def test_delete_result_map_by_id(delete_result_map_by_id_data, caplog):
    caplog.set_level(logging.INFO)
    runner = CliRunner()
    # Include interactive input and expected message if this test should trigger interactive stuff
    if "interactive" in delete_result_map_by_id_data:
        expected_output = (
            delete_result_map_by_id_data["interactive"]["message"]
            + delete_result_map_by_id_data["return"]
            + "\n"
        )
        result = runner.invoke(
            carrot,
            delete_result_map_by_id_data["args"],
            input=delete_result_map_by_id_data["interactive"]["input"],
        )
        assert result.output == expected_output
    else:
        result = runner.invoke(carrot, delete_result_map_by_id_data["args"])
        assert result.output == delete_result_map_by_id_data["return"] + "\n"
    # If we expect logging that we want to check, make sure it's there
    if "logging" in delete_result_map_by_id_data:
        assert delete_result_map_by_id_data["logging"] in caplog.text


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "map_to_report",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single",
                "adora@example.com",
            ],
            "return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "map_to_report",
                "Horde Template",
                "Horde Report",
                "pr",
                "--created_by",
                "adora@example.com",
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "pr",
                "adora@example.com",
            ],
            "from_names": {
                "report_name": "Horde Report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This old report is old",
                            "name": "Horde Report",
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
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "pr",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "map_to_report",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "params": [],
            "logging": "No email config variable set.  If a value is not specified for --created_by, "
            "there must be a value set for email.",
        },
        {
            "args": ["template", "map_to_report"],
            "params": [],
            "return": "Usage: carrot_cli template map_to_report [OPTIONS] TEMPLATE REPORT [[single|pr]]\n"
            "Try 'carrot_cli template map_to_report -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def map_to_report_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_reports).create_map(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
        mockito.when(templates).find(
            name=request.param["from_names"]["template_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["template_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(template_reports).create_map(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
            request.param["params"][3]
        ).thenReturn(request.param["return"])
    return request.param


def test_map_to_report(map_to_report_data, caplog):
    runner = CliRunner()
    result = runner.invoke(carrot, map_to_report_data["args"])
    if "logging" in map_to_report_data:
        assert map_to_report_data["logging"] in caplog.text
    else:
        assert result.output == map_to_report_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "find_report_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "input_map": {"section1": {"input1": "val1"}},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find_report_map_by_id",
                "Horde Template",
                "Horde Report",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "from_names": {
                "report_name": "Horde Report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This old report is old",
                            "name": "Horde Report",
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
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "input_map": {"section1": {"input1": "val1"}},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": ["template", "find_report_map_by_id"],
            "params": [],
            "return": "Usage: carrot_cli template find_report_map_by_id [OPTIONS] TEMPLATE REPORT\n"
            "                                                 {single|pr}\n"
            "Try 'carrot_cli template find_report_map_by_id -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def find_report_map_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_reports).find_map_by_ids(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
        mockito.when(templates).find(
            name=request.param["from_names"]["template_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["template_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(template_reports).find_map_by_ids(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2]
        ).thenReturn(request.param["return"])
    return request.param


def test_find_report_map_by_id(find_report_map_by_id_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_report_map_by_id_data["args"])
    assert result.output == find_report_map_by_id_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "find_report_maps",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--report_id",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--report_trigger",
                "single",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(input_map)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "adora@example.com",
                "asc(input_map)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        "report_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "report_trigger": "single",
                        "created_at": "2020-09-24T19:07:59.311462",
                        "created_by": "adora@example.com",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find_report_maps",
                "Horde Template",
                "--report",
                "Horde Report",
                "--report_trigger",
                "single",
                "--created_by",
                "adora@example.com",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(input_map)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "adora@example.com",
                "asc(input_map)",
                1,
                0,
            ],
            "from_names": {
                "report_name": "Horde Report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This old report is old",
                            "name": "Horde Report",
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
                            "pipeline_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "return": json.dumps(
                [
                    {
                        "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        "report_id": "4d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "report_trigger": "single",
                        "created_at": "2020-09-24T19:07:59.311462",
                        "created_by": "adora@example.com",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "template",
                "find_report_maps",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
                None,
                None,
                None,
                None,
                None,
                None,
                20,
                0,
            ],
            "return": json.dumps(
                {
                    "title": "No template_reports found",
                    "status": 404,
                    "detail": "No template_reports found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_report_maps_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_reports).find_maps(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
        mockito.when(templates).find(
            name=request.param["from_names"]["template_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["template_return"])
    # Mock up request response
    mockito.when(template_reports).find_maps(
        request.param["params"][0],
        request.param["params"][1],
        request.param["params"][2],
        request.param["params"][3],
        request.param["params"][4],
        request.param["params"][5],
        request.param["params"][6],
        request.param["params"][7],
        request.param["params"][8],
    ).thenReturn(request.param["return"])
    return request.param


def test_find_report_maps(find_report_maps_data):
    runner = CliRunner()
    result = runner.invoke(carrot, find_report_maps_data["args"])
    assert result.output == find_report_maps_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "template",
                "delete_report_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "rogelio@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "template",
                "delete_report_map_by_id",
                "Horde Template",
                "Horde Report",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
            "from_names": {
                "report_name": "Horde Report",
                "report_return": json.dumps(
                    [
                        {
                            "config": {"cpu": 2},
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "description": "This old report is old",
                            "name": "Horde Report",
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
                            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                        }
                    ],
                    indent=4,
                    sort_keys=True,
                )
            },
            "email": "rogelio@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "args": [
                "template",
                "delete_report_map_by_id",
                "-y",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
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
                "template",
                "delete_report_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
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
                "message": "Mapping for template with id cd987859-06fe-4b1a-9e96-47d4f36bf819 and "
                "report with id 3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8 triggered by single was created by "
                "adora@example.com. Are you sure you want to delete? [y/N]: y\n",
            },
        },
        {
            "args": [
                "template",
                "delete_report_map_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "single"
            ],
            "find_return": json.dumps(
                {
                    "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "report_trigger": "single",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "adora@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
            "email": "catra@example.com",
            "return": "",
            "interactive": {
                "input": "n",
                "message": "Mapping for template with id cd987859-06fe-4b1a-9e96-47d4f36bf819 and "
                "report with id 3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8 triggered by single was created by "
               "adora@example.com. Are you sure you want to delete? [y/N]: n",
            },
            "logging": "Okay, aborting delete operation",
        },
        {
            "args": ["template", "delete_report_map_by_id"],
            "params": [],
            "email": "rogelio@example.com",
            "return": "Usage: carrot_cli template delete_report_map_by_id [OPTIONS] TEMPLATE REPORT\n"
                      "                                                   {single|pr}\n"
            "Try 'carrot_cli template delete_report_map_by_id -h' for help.\n"
            "\n"
            "Error: Missing argument 'TEMPLATE'.",
        },
    ]
)
def delete_report_map_by_id_data(request):
    # We want to load the value from "email" from config
    mockito.when(config).load_var("email").thenReturn(request.param["email"])
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(template_reports).delete_map_by_ids(...).thenReturn(None)
    mockito.when(template_reports).find_map_by_ids(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_names" in request.param:
        mockito.when(reports).find(
            name=request.param["from_names"]["report_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["report_return"])
        mockito.when(templates).find(
            name=request.param["from_names"]["template_name"],
            limit=2
        ).thenReturn(request.param["from_names"]["template_return"])
    # Mock up request response only if we expect it to get that far
    if len(request.param["params"]) > 0:
        mockito.when(template_reports).delete_map_by_ids(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
        ).thenReturn(request.param["return"])
        mockito.when(template_reports).find_map_by_ids(
            request.param["params"][0],
            request.param["params"][1],
            request.param["params"][2],
        ).thenReturn(request.param["find_return"])
    return request.param


def test_delete_report_map_by_id(delete_report_map_by_id_data, caplog):
    caplog.set_level(logging.INFO)
    runner = CliRunner()
    # Include interactive input and expected message if this test should trigger interactive stuff
    if "interactive" in delete_report_map_by_id_data:
        expected_output = (
            delete_report_map_by_id_data["interactive"]["message"]
            + delete_report_map_by_id_data["return"]
            + "\n"
        )
        result = runner.invoke(
            carrot,
            delete_report_map_by_id_data["args"],
            input=delete_report_map_by_id_data["interactive"]["input"],
        )
        assert result.output == expected_output
    else:
        result = runner.invoke(carrot, delete_report_map_by_id_data["args"])
        assert result.output == delete_report_map_by_id_data["return"] + "\n"
    # If we expect logging that we want to check, make sure it's there
    if "logging" in delete_report_map_by_id_data:
        assert delete_report_map_by_id_data["logging"] in caplog.text
