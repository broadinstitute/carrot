import json

import mockito
import pytest
from carrot_cli.rest import request_handler, template_results


@pytest.fixture(
    params=[
        {
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "result_key": "out_horde_tanks",
            "created_by": "rogelio@example.com",
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
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "result_key": "out_horde_tanks",
            "created_by": "rogelio@example.com",
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new template result mapping",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def create_map_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).create_map(...).thenReturn(None)
    # Mock up request response
    params = [
        ("result_key", request.param["result_key"]),
        ("created_by", request.param["created_by"]),
    ]
    mockito.when(request_handler).create_map(
        "templates",
        request.param["template_id"],
        "results",
        request.param["result_id"],
        params,
    ).thenReturn(request.param["return"])
    return request.param


def test_create_map(create_map_data):
    result = template_results.create_map(
        create_map_data["template_id"],
        create_map_data["result_id"],
        create_map_data["result_key"],
        create_map_data["created_by"],
    )
    assert result == create_map_data["return"]


@pytest.fixture(
    params=[
        {
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("result_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("result_key", "out_horde_tanks"),
                ("created_before", ""),
                ("created_after", ""),
                ("created_by", "rogelio@example.com"),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
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
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("result_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("result_key", "out_horde_tanks"),
                ("created_before", ""),
                ("created_after", ""),
                ("created_by", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "title": "No template_result mapping found",
                    "status": 404,
                    "detail": "No template_result mapping found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_maps_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).find_maps(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find_maps(
        "templates", request.param["template_id"], "results", request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_maps(find_maps_data):
    result = template_results.find_maps(
        find_maps_data["template_id"],
        find_maps_data["params"][0][1],
        find_maps_data["params"][1][1],
        find_maps_data["params"][2][1],
        find_maps_data["params"][3][1],
        find_maps_data["params"][4][1],
        find_maps_data["params"][5][1],
        find_maps_data["params"][6][1],
        find_maps_data["params"][7][1],
    )
    assert result == find_maps_data["return"]


@pytest.fixture(
    params=[
        {
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No template_result mapping found",
                    "status": 404,
                    "detail": "No template_result mapping found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_map_by_ids_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).find_map_by_ids(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find_map_by_ids(
        "templates", request.param["template_id"], "results", request.param["result_id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_maps_by_id(find_map_by_ids_data):
    result = template_results.find_map_by_ids(
        find_map_by_ids_data["template_id"], find_map_by_ids_data["result_id"]
    )
    assert result == find_map_by_ids_data["return"]


@pytest.fixture(
    params=[
        {
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "template_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "result_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No template_result mapping found",
                    "status": 404,
                    "detail": "No template_result mapping found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def delete_map_by_ids_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).delete_map_by_ids(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).delete_map_by_ids(
        "templates", request.param["template_id"], "results", request.param["result_id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_delete_maps_by_id(delete_map_by_ids_data):
    result = template_results.delete_map_by_ids(
        delete_map_by_ids_data["template_id"], delete_map_by_ids_data["result_id"]
    )
    assert result == delete_map_by_ids_data["return"]
