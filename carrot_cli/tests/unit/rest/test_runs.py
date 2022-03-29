import json

import mockito
import pytest
from carrot_cli.rest import request_handler, runs


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
                    "finished_at": None,
                    "created_by": "adora@example.com",
                    "test_input": {"in_prev_owner": "Mara"},
                    "eval_input": {"in_creators": "Old Ones"},
                    "status": "testsubmitted",
                    "results": {},
                    "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                    "eval_cromwell_job_id": None,
                    "name": "Sword of Protection run",
                    "test_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "run_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No run found",
                    "status": 404,
                    "detail": "No run found with the specified ID",
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
    mockito.when(request_handler).find_by_id("runs", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    result = runs.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "parent_entity": "tests",
            "parent_entity_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
            "params": [
                ("name", "Queen of Bright Moon run"),
                ("status", ""),
                ("test_input", ""),
                ("test_options", ""),
                ("eval_input", ""),
                ("eval_options", ""),
                ("test_cromwell_job_id", ""),
                ("eval_cromwell_job_id", ""),
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
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": None,
                        "created_by": "glimmer@example.com",
                        "test_input": {"in_daughter": "Glimmer"},
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_wife": "Angella"},
                        "eval_options": {"option": "value"},
                        "status": "testsubmitted",
                        "results": {},
                        "test_cromwell_job_id": "d9855002-6b71-429c-a4de-8e90222488cd",
                        "eval_cromwell_job_id": None,
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
            "parent_entity": "tests",
            "parent_entity_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
            "params": [
                ("name", "Queen of Bright Moon run"),
                ("status", ""),
                ("test_input", ""),
                ("test_options", ""),
                ("eval_input", ""),
                ("eval_options", ""),
                ("test_cromwell_job_id", ""),
                ("eval_cromwell_job_id", ""),
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
                    "title": "No runs found",
                    "status": 404,
                    "detail": "No runs found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).find_runs(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).find_runs(
        request.param["parent_entity"],
        request.param["parent_entity_id"],
        request.param["params"],
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    result = runs.find(
        find_data["parent_entity"],
        find_data["parent_entity_id"],
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
        find_data["params"][14][1],
        find_data["params"][15][1],
    )
    assert result == find_data["return"]


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
                    "title": "No run found",
                    "status": 404,
                    "detail": "No run found with the specified ID",
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
    mockito.when(request_handler).delete("runs", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    result = runs.delete(delete_data["id"])
    assert result == delete_data["return"]
