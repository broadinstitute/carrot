import json

import mockito
import pytest
from carrot_cli.rest import request_handler, results


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
                    "description": "This result will save Etheria",
                    "name": "Sword of Protection result",
                    "result_type": "file",
                    "result_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
    mockito.when(request_handler).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find_by_id("results", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    result = results.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("result_id", ""),
                ("name", "Queen of Bright Moon result"),
                ("description", ""),
                ("result_type", ""),
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
                        "description": "This result leads the Rebellion",
                        "name": "Queen of Bright Moon result",
                        "result_type": "text",
                        "result_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("result_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("name", ""),
                ("description", ""),
                ("result_type", ""),
                ("created_by", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
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
    mockito.when(request_handler).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find("results", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find(find_data):
    result = results.find(
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
    )
    assert result == find_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("name", "Horde Emperor result"),
                ("description", "This result rules the known universe"),
                ("result_type", "file"),
                ("created_by", "hordeprime@example.com"),
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "hordeprime@example.com",
                    "description": "This result rules the known universe",
                    "result_type": "file",
                    "name": "Horde Emperor result",
                    "result_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("name", "Horde Emperor result"),
                ("description", "This result rules the known universe"),
                ("result_type", "numeric"),
                ("created_by", "hordeprime@example.com"),
            ],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new result",
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
    mockito.when(request_handler).create("results", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_create(create_data):
    result = results.create(
        create_data["params"][0][1],
        create_data["params"][1][1],
        create_data["params"][2][1],
        create_data["params"][3][1],
    )
    assert result == create_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Catra result"),
                (
                    "description",
                    "This result is trying to learn to process anger better",
                ),
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "catra@example.com",
                    "description": "This result is trying to learn to process anger better",
                    "name": "Catra result",
                    "result_type": "numeric",
                    "result_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "params": [("name", "Angella result"), ("description", "")],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to update new result",
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
        "results", request.param["id"], request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    result = results.update(
        update_data["id"],
        update_data["params"][0][1],
        update_data["params"][1][1],
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
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).delete(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).delete("results", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    result = results.delete(delete_data["id"])
    assert result == delete_data["return"]
