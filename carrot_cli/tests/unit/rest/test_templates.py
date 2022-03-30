import builtins
import json

import mockito
import pytest
from carrot_cli.rest import request_handler, templates


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
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
    mockito.when(request_handler).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find_by_id(
        "templates", request.param["id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_by_id(find_by_id_data):
    result = templates.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("template_id", ""),
                ("pipeline_id", ""),
                ("name", "Queen of Bright Moon template"),
                ("description", ""),
                ("test_wdl", ""),
                ("eval_wdl", ""),
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
                        "description": "This template leads the Rebellion",
                        "test_wdl": "http://example.com/etheria_test.wdl",
                        "eval_wdl": "http://example.com/etheria_eval.wdl",
                        "name": "Queen of Bright Moon template",
                        "pipeline_id": "58723b05-6060-4444-9f1b-394aff691cce",
                        "template_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("template_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("pipeline_id", ""),
                ("name", ""),
                ("description", ""),
                ("test_wdl", ""),
                ("eval_wdl", ""),
                ("created_by", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
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
    mockito.when(request_handler).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find("templates", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find(find_data):
    result = templates.find(
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
        find_data["params"][11][1],
    )
    assert result == find_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("name", "Horde Emperor template"),
                ("pipeline_id", "9d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("description", "This template rules the known universe"),
                ("created_by", "hordeprime@example.com"),
                ("test_wdl", "http://example.com/horde_test.wdl"),
                ("test_wdl_dependencies", "http://example.com/horde_test_dependencies.zip"),
                ("eval_wdl", "http://example.com/horde_eval.wdl"),
                ("eval_wdl_dependencies", "http://example.com/horde_eval_dependencies.zip"),
            ],
            "has_files": False,
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "hordeprime@example.com",
                    "test_wdl": "http://example.com/horde_test.wdl",
                    "test_wdl_dependencies": "http://example.com/horde_test_dependencies.zip",
                    "eval_wdl": "http://example.com/horde_eval.wdl",
                    "eval_wdl_dependencies": "http://example.com/horde_eval_dependencies.zip",
                    "description": "This template rules the known universe",
                    "name": "Horde Emperor template",
                    "pipeline_id": "9d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("name", "Horde Emperor template"),
                ("pipeline_id", "9d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("description", "This template rules the known universe"),
                ("created_by", "hordeprime@example.com"),
                ("test_wdl", "tests/data/test.wdl"),
                ("test_wdl_dependencies", "tests/data/test_dep.zip"),
                ("eval_wdl", "tests/data/eval.wdl"),
                ("eval_wdl_dependencies", "tests/data/eval_dep.zip"),
            ],
            "has_files": True,
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "hordeprime@example.com",
                    "test_wdl": "http://example.com/horde_test.wdl",
                    "test_wdl_dependencies": "http://example.com/horde_test_dependencies.zip",
                    "eval_wdl": "http://example.com/horde_eval.wdl",
                    "eval_wdl_dependencies": "http://example.com/horde_eval_dependencies.zip",
                    "description": "This template rules the known universe",
                    "name": "Horde Emperor template",
                    "pipeline_id": "9d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("name", "Horde Emperor template"),
                ("pipeline_id", "9d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("description", "This template rules the known universe"),
                ("created_by", "hordeprime@example.com"),
                ("test_wdl", "http://example.com/horde_test.wdl"),
                ("test_wdl_dependencies", "http://example.com/horde_test_dependencies.zip"),
                ("eval_wdl", "http://example.com/horde_eval.wdl"),
                ("eval_wdl_dependencies", "http://example.com/horde_eval_dependencies.zip"),
            ],
            "has_files": False,
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new template",
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
    # If we're testing files, put dummy values for them in a dict
    if request.param["has_files"]:
        params = request.param["params"].copy()
        files = {}
        if not request.param["params"][4][1].startswith("http"):
            params.remove(request.param["params"][4])
            files['test_wdl_file'] = request.param["params"][4][1]
        if not request.param["params"][5][1].startswith("http"):
            params.remove(request.param["params"][5])
            files['test_wdl_dependencies_file'] = request.param["params"][5][1]
        if not request.param["params"][6][1].startswith("http"):
            params.remove(request.param["params"][6])
            files['eval_wdl_file'] = request.param["params"][6][1]
        if not request.param["params"][7][1].startswith("http"):
            params.remove(request.param["params"][7])
            files['eval_wdl_dependencies_file'] = request.param["params"][7][1]
        # Mock up request response
        mockito.when(request_handler).create(
            "templates", params, files=files
        ).thenReturn(request.param["return"])
    # Otherwise, don't pass any files
    else:
        # Mock up request response
        mockito.when(request_handler).create(
            "templates", request.param["params"], files=None
        ).thenReturn(request.param["return"])
    return request.param


def test_create(create_data):
    result = templates.create(
        create_data["params"][0][1],
        create_data["params"][1][1],
        create_data["params"][2][1],
        create_data["params"][4][1],
        create_data["params"][5][1],
        create_data["params"][6][1],
        create_data["params"][7][1],
        create_data["params"][3][1],
    )
    assert result == create_data["return"]

@pytest.fixture(
    params=[
        {
            "id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Catra template"),
                (
                    "description",
                    "This template is trying to learn to process anger better",
                ),
                ("test_wdl", "http://example.com/horde_test.wdl"),
                ("test_wdl_dependencies", "http://example.com/horde_test_dep.zip"),
                ("eval_wdl", "http://example.com/horde_eval.wdl"),
                ("eval_wdl_dependencies", "http://example.com/horde_eval_dep.zip"),
            ],
            "has_files": False,
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "catra@example.com",
                    "test_wdl": "http://example.com/horde_test.wdl",
                    "eval_wdl": "http://example.com/horde_eval.wdl",
                    "description": "This template is trying to learn to process anger better",
                    "name": "Catra template",
                    "pipeline_id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
                    "template_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Catra template"),
                (
                    "description",
                    "This template is trying to learn to process anger better",
                ),
                ("test_wdl", "tests/data/test.wdl"),
                ("test_wdl_dependencies", "tests/data/test_dep.zip"),
                ("eval_wdl", "tests/data/eval.wdl"),
                ("eval_wdl_dependencies", "tests/data/eval_dep.zip"),
            ],
            "has_files": True,
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "catra@example.com",
                    "test_wdl": "http://example.com/horde_test.wdl",
                    "eval_wdl": "http://example.com/horde_eval.wdl",
                    "description": "This template is trying to learn to process anger better",
                    "name": "Catra template",
                    "pipeline_id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
                    "template_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Catra template"),
                (
                    "description",
                    "This template is trying to learn to process anger better",
                ),
                ("test_wdl", "http://example.com/horde_test.wdl"),
                ("test_wdl_dependencies", "http://example.com/horde_test_dep.zip"),
                ("eval_wdl", "http://example.com/horde_eval.wdl"),
                ("eval_wdl_dependencies", "http://example.com/horde_eval_dep.zip"),
            ],
            "has_files": False,
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to update template",
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
    # If we're testing files, put dummy values for them in a dict
    if request.param["has_files"]:
        params = request.param["params"].copy()
        files = {}
        if not request.param["params"][2][1].startswith("http"):
            params.remove(request.param["params"][2])
            files['test_wdl_file'] = request.param["params"][2][1]
        if not request.param["params"][3][1].startswith("http"):
            params.remove(request.param["params"][3])
            files['test_wdl_dependencies_file'] = request.param["params"][3][1]
        if not request.param["params"][4][1].startswith("http"):
            params.remove(request.param["params"][4])
            files['eval_wdl_file'] = request.param["params"][4][1]
        if not request.param["params"][5][1].startswith("http"):
            params.remove(request.param["params"][5])
            files['eval_wdl_dependencies_file'] = request.param["params"][5][1]
        # Mock up request response
        mockito.when(request_handler).update(
            "templates", request.param["id"], params, files=files
        ).thenReturn(request.param["return"])
    # Otherwise, don't pass any files
    else:
        # Mock up request response
        mockito.when(request_handler).update(
            "templates", request.param["id"], request.param["params"], files=None
        ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    result = templates.update(
        update_data["id"],
        update_data["params"][0][1],
        update_data["params"][1][1],
        update_data["params"][2][1],
        update_data["params"][3][1],
        update_data["params"][4][1],
        update_data["params"][5][1],
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
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).delete(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).delete("templates", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    result = templates.delete(delete_data["id"])
    assert result == delete_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
            "email": "bow@example.com",
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "template",
                    "entity_id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
                    "email": "bow@example.com",
                    "created_at": "2020-09-23T19:41:46.839880",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "email": "huntara@example.com",
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
def subscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).update(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).subscribe(
        "templates", request.param["id"], request.param["email"]
    ).thenReturn(request.param["return"])
    return request.param


def test_subscribe(subscribe_data):
    result = templates.subscribe(
        subscribe_data["id"],
        subscribe_data["email"],
    )
    assert result == subscribe_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
            "email": "mermista@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "email": "castaspella@example.com",
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
    ]
)
def unsubscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).update(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).unsubscribe(
        "templates", request.param["id"], request.param["email"]
    ).thenReturn(request.param["return"])
    return request.param


def test_unsubscribe(unsubscribe_data):
    result = templates.unsubscribe(
        unsubscribe_data["id"],
        unsubscribe_data["email"],
    )
    assert result == unsubscribe_data["return"]

@pytest.fixture(
    params=[
        {
            "params": [
                ("name", "Template name")
            ],
            "files": {},
            "field_val": "http://example.com/test.wdl",
            "field_name": "test_wdl",
            "add_to_params": True,
            "add_to_files": False,
        },
        {
            "params": [
                ("name", "Template name")
            ],
            "files": {},
            "field_val": "tests/data/eval.wdl",
            "field_name": "eval_wdl",
            "add_to_params": False,
            "add_to_files": True,
        },
        {
            "params": [
                ("name", "Template name")
            ],
            "files": {
                "test_wdl_file": "tests/data/test.wdl"
            },
            "field_val": "tests/data/eval_deps.zip",
            "field_name": "eval_wdl_dependencies_file",
            "add_to_params": False,
            "add_to_files": True,
        },
    ]
)
def process_maybe_file_field_data(request):
    return request.param


def test_process_maybe_file_field(process_maybe_file_field_data):
    # Copy the params and files so we can make sure they're not changed if we don't want them to be
    params = process_maybe_file_field_data["params"].copy()
    files = process_maybe_file_field_data["files"].copy()
    templates.__process_maybe_file_field(
        process_maybe_file_field_data["params"],
        process_maybe_file_field_data["files"],
        process_maybe_file_field_data["field_name"],
        process_maybe_file_field_data["field_val"]
    )
    if process_maybe_file_field_data["add_to_params"]:
        assert (process_maybe_file_field_data['field_name'], process_maybe_file_field_data["field_val"]) in process_maybe_file_field_data["params"]
    else:
        assert process_maybe_file_field_data["params"] == params
    if process_maybe_file_field_data["add_to_files"]:
        assert f"{process_maybe_file_field_data['field_name']}_file" in process_maybe_file_field_data["files"]
        assert process_maybe_file_field_data["files"][f"{process_maybe_file_field_data['field_name']}_file"] == process_maybe_file_field_data["field_val"]
    else:
        assert process_maybe_file_field_data["files"] == files
