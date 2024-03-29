import json

import copy
import mockito
import pytest
from carrot_cli.rest import request_handler, report_maps


@pytest.fixture(
    params=[
        {
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "entity": "runs",
            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "created_by": "rogelio@example.com",
            "delete_failed": True,
            "return": json.dumps(
                {
                    "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "entity_type": "run",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "created",
                    "results": {},
                    "cromwell_job_id": "8f1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                    "finished_at": None,
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "entity": "runs",
            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "created_by": "rogelio@example.com",
            "delete_failed": False,
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new run report mapping",
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
        ("created_by", request.param["created_by"]),
    ]
    query_params = [
        ("delete_failed", "true" if request.param["delete_failed"] else "false")
    ]
    mockito.when(request_handler).create_map(
        request.param["entity"],
        request.param["entity_id"],
        "reports",
        request.param["report_id"],
        params,
        query_params,
    ).thenReturn(request.param["return"])
    return request.param


def test_create_map(create_map_data):
    report = report_maps.create_map(
        create_map_data["entity"],
        create_map_data["entity_id"],
        create_map_data["report_id"],
        create_map_data["created_by"],
        create_map_data["delete_failed"],
    )
    assert report == create_map_data["return"]


@pytest.fixture(
    params=[
        {
            "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "parent_entity": "runs",
            "parent_entity_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "created_by": "rogelio@example.com",
            "params": [
                ("run_group_id", None),
                ("name", None),
                ("status", None),
                ("test_input", None),
                ("test_options", None),
                ("eval_input", None),
                ("eval_options", None),
                ("test_cromwell_job_id", None),
                ("eval_cromwell_job_id", None),
                ("software_versions", {"name": "test_software", "commits_and_tags": ["1.1.0"]}),
                ("created_before", None),
                ("created_after", None),
                ("created_by", None),
                ("finished_before", None),
                ("finished_after", None),
                ("sort", None),
                ("limit", None),
                ("offset", None),
            ],
            "return": json.dumps(
                {
                    "entity_id": "128abc85-06fe-4b1a-9e96-47d4f36bf819",
                    "entity_type": "run_group",
                    "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "status": "created",
                    "results": {},
                    "cromwell_job_id": "8f1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                    "finished_at": None,
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "report_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "parent_entity": "runs",
            "parent_entity_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "created_by": "rogelio@example.com",
            "params": [
                ("run_group_id", None),
                ("name", None),
                ("status", None),
                ("test_input", None),
                ("test_options", None),
                ("eval_input", None),
                ("eval_options", None),
                ("test_cromwell_job_id", None),
                ("eval_cromwell_job_id", None),
                ("software_versions", {"name": "test_software", "commits_and_tags": ["1.1.0"]}),
                ("created_before", None),
                ("created_after", None),
                ("created_by", None),
                ("finished_before", None),
                ("finished_after", None),
                ("sort", None),
                ("limit", None),
                ("offset", None),
            ],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new run report mapping",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def create_map_from_run_query_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).create_report_map_from_run_query(...).thenReturn(None)
    # Mock up request response
    body_params = [
        ("created_by", request.param["created_by"]),
    ]
    params_for_mock = copy.deepcopy(request.param["params"])
    params_for_mock[9] = ("software_versions", json.dumps(params_for_mock[9][1]) if params_for_mock[9][1] else None)
    mockito.when(request_handler).create_report_map_from_run_query(
        request.param["report_id"],
        request.param["parent_entity"],
        request.param["parent_entity_id"],
        body_params,
        params_for_mock,
    ).thenReturn(request.param["return"])
    return request.param


def test_create_map_from_run_query(create_map_from_run_query_data):
    report = report_maps.create_map_from_run_query(
        create_map_from_run_query_data["report_id"],
        create_map_from_run_query_data["created_by"],
        create_map_from_run_query_data["parent_entity"],
        create_map_from_run_query_data["parent_entity_id"],
        create_map_from_run_query_data["params"][0][1],
        create_map_from_run_query_data["params"][1][1],
        create_map_from_run_query_data["params"][2][1],
        create_map_from_run_query_data["params"][3][1],
        create_map_from_run_query_data["params"][4][1],
        create_map_from_run_query_data["params"][5][1],
        create_map_from_run_query_data["params"][6][1],
        create_map_from_run_query_data["params"][7][1],
        create_map_from_run_query_data["params"][8][1],
        create_map_from_run_query_data["params"][9][1],
        create_map_from_run_query_data["params"][10][1],
        create_map_from_run_query_data["params"][11][1],
        create_map_from_run_query_data["params"][12][1],
        create_map_from_run_query_data["params"][13][1],
        create_map_from_run_query_data["params"][14][1],
        create_map_from_run_query_data["params"][15][1],
        create_map_from_run_query_data["params"][16][1],
        create_map_from_run_query_data["params"][17][1],
    )
    assert report == create_map_from_run_query_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "runs",
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("report_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("status", ""),
                ("cromwell_job_id", ""),
                ("results", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("created_by", "rogelio@example.com"),
                ("finished_before", ""),
                ("finished_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "entity_type": "run",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                    "finished_at": "2020-09-24T19:09:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "entity": "runs",
            "params": [
                ("report_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("status", ""),
                ("cromwell_job_id", ""),
                ("results", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("created_by", ""),
                ("finished_before", ""),
                ("finished_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "title": "No report_map mapping found",
                    "status": 404,
                    "detail": "No report_map mapping found with the specified parameters",
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
        request.param["entity"], request.param["entity_id"], "reports", request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_maps(find_maps_data):
    report = report_maps.find_maps(
        find_maps_data["entity"],
        find_maps_data["entity_id"],
        find_maps_data["params"][0][1],
        find_maps_data["params"][1][1],
        find_maps_data["params"][2][1],
        find_maps_data["params"][3][1],
        find_maps_data["params"][4][1],
        find_maps_data["params"][5][1],
        find_maps_data["params"][6][1],
        find_maps_data["params"][7][1],
        find_maps_data["params"][8][1],
        find_maps_data["params"][9][1],
        find_maps_data["params"][10][1],
        find_maps_data["params"][11][1],
    )
    assert report == find_maps_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "runs",
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "entity_type": "run",
                    "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "status": "succeeded",
                    "cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "results": {"result1": "val1"},
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                    "finished_at": "2020-09-24T21:07:59.311462",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity": "runs",
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No report_map mapping found",
                    "status": 404,
                    "detail": "No report_map mapping found with the specified ID",
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
        request.param["entity"], request.param["entity_id"], "reports", request.param["report_id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_maps_by_id(find_map_by_ids_data):
    report = report_maps.find_map_by_ids(
        find_map_by_ids_data["entity"], find_map_by_ids_data["entity_id"], find_map_by_ids_data["report_id"]
    )
    assert report == find_map_by_ids_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "runs",
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "entity": "runs",
            "entity_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "report_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No report_map mapping found",
                    "status": 404,
                    "detail": "No report_map mapping found with the specified ID",
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
        request.param["entity"], request.param["entity_id"], "reports", request.param["report_id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_delete_maps_by_id(delete_map_by_ids_data):
    report = report_maps.delete_map_by_ids(
        delete_map_by_ids_data["entity"], delete_map_by_ids_data["entity_id"], delete_map_by_ids_data["report_id"]
    )
    assert report == delete_map_by_ids_data["return"]
