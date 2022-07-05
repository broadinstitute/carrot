import json

import mockito
import pytest
from carrot_cli.rest import request_handler, run_groups

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
                    "author": "example_user",
                    "base_commit": "13c988d4f15e06bcdd0b0af290086a3079cdadb0",
                    "created_at": "2020-09-16T18:48:06.371563",
                    "created_by": "adora@example.com",
                    "eval_input_key": "workflow.key",
                    "head_commit": "d240853866f20fc3e536cb3bca86c86c54b723ce",
                    "issue_number": 14,
                    "name": "Sword of Protection result",
                    "owner": "me",
                    "repo": "example_software",
                    "run_group_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "test_input_key": "test.key"
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No run group found",
                    "status": 404,
                    "detail": "No run group found with the specified ID",
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
    mockito.when(request_handler).find_by_id("run-groups", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    result = run_groups.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]

@pytest.fixture(
    params=[
        {
            "params": [
                ("run_group_id", ""),
                ("owner", "WhatACoolExampleOrganization"),
                ("repo", ""),
                ("issue_number", ""),
                ("author", ""),
                ("base_commit", ""),
                ("head_commit", ""),
                ("test_input_key", ""),
                ("eval_input_key", ""),
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
                        "owner": "WhatACoolExampleOrganization",
                        "repo": "WhatACoolExampleRepo",
                        "issue_number": 12,
                        "author": "WhatACoolExampleUser",
                        "base_commit": "13c988d4f15e06bcdd0b0af290086a3079cdadb0",
                        "head_commit": "d240853866f20fc3e536cb3bca86c86c54b723ce",
                        "test_input_key": "example_workflow.docker_key",
                        "eval_input_key": None,
                        "run_group_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("run_group_id", ""),
                ("owner", "WhatACoolExampleOrganization"),
                ("repo", ""),
                ("issue_number", ""),
                ("author", ""),
                ("base_commit", ""),
                ("head_commit", ""),
                ("test_input_key", ""),
                ("eval_input_key", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "title": "No run groups found",
                    "status": 404,
                    "detail": "No run groups found with the specified parameters",
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
    mockito.when(request_handler).find("run-groups", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find(find_data):
    run_group = run_groups.find(
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
        find_data["params"][12][1],
        find_data["params"][13][1],
    )
    assert run_group == find_data["return"]

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
                    "title": "No run group found",
                    "status": 404,
                    "detail": "No run group found with the specified ID",
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
    mockito.when(request_handler).delete("run-groups", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    run_group = run_groups.delete(delete_data["id"])
    assert run_group == delete_data["return"]
