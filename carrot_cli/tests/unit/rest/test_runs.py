import copy
import json
import tempfile

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
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "csv": True,
            "file_contents": b"randombytesrepresentingazip",
            "return": "Success!",
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).find_by_id(...).thenReturn(None)
    # Mock up request response
    if "csv" in request.param and request.param["csv"]:
        mockito.when(request_handler).find_by_id(
            "runs",
            request.param["id"],
            params=[("csv", str(request.param["csv"]).lower())],
            expected_format=request_handler.ResponseFormat.BYTES,
        ).thenReturn(request.param["file_contents"])
        # Create a temp file that we'll write to
        request.param["file"] = tempfile.NamedTemporaryFile()
        request.param["csv"] = request.param["file"].name
    else:
        mockito.when(request_handler).find_by_id(
            "runs", request.param["id"]
        ).thenReturn(request.param["return"])
    return request.param


def test_find_by_id(find_by_id_data):
    if "csv" in find_by_id_data:
        result = runs.find_by_id(find_by_id_data["id"], csv=find_by_id_data["csv"])
        # Check that the file has the correct data written to it
        written_data = find_by_id_data["file"].read()
        assert written_data == find_by_id_data["file_contents"]
    else:
        result = runs.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "parent_entity": "tests",
            "parent_entity_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
            "params": [
                ("run_group_id", None),
                ("name", "Queen of Bright Moon run"),
                ("status", None),
                ("test_input", None),
                ("test_options", None),
                ("eval_input", None),
                ("eval_options", None),
                ("test_cromwell_job_id", None),
                ("eval_cromwell_job_id", None),
                ("software_versions", None),
                ("created_before", None),
                ("created_after", None),
                ("created_by", None),
                ("finished_before", None),
                ("finished_after", None),
                ("sort", None),
                ("limit", None),
                ("offset", None),
                ("csv", False),
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
                        "run_group_id": None,
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
                ("run_group_id", None),
                ("name", None),
                ("status", None),
                ("test_input", None),
                ("test_options", None),
                ("eval_input", None),
                ("eval_options", None),
                ("test_cromwell_job_id", None),
                ("eval_cromwell_job_id", None),
                (
                    "software_versions",
                    {"name": "test_software", "commits_and_tags": ["1.1.0"]},
                ),
                ("created_before", None),
                ("created_after", None),
                ("created_by", None),
                ("finished_before", None),
                ("finished_after", None),
                ("sort", None),
                ("limit", None),
                ("offset", None),
                ("csv", False),
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "finished_at": None,
                        "created_by": "glimmer@example.com",
                        "test_input": {
                            "in_daughter": "Glimmer",
                            "docker": "image_build:test_software|1.1.0",
                        },
                        "test_options": {"option": "other_value"},
                        "eval_input": {"in_wife": "Angella"},
                        "eval_options": {"option": "value"},
                        "status": "testsubmitted",
                        "results": {},
                        "run_group_id": None,
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
                ("run_group_id", None),
                ("name", "Queen of Bright Moon run"),
                ("status", None),
                ("test_input", None),
                ("test_options", None),
                ("eval_input", None),
                ("eval_options", None),
                ("test_cromwell_job_id", None),
                ("eval_cromwell_job_id", None),
                ("software_versions", None),
                ("created_before", None),
                ("created_after", None),
                ("created_by", None),
                ("finished_before", None),
                ("finished_after", None),
                ("sort", None),
                ("limit", None),
                ("offset", None),
                ("csv", False),
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
        {
            "parent_entity": "tests",
            "parent_entity_id": "dd1b6094-b43a-4d98-8873-cc9b38e8b85d",
            "params": [
                ("run_group_id", None),
                ("name", "Queen of Bright Moon run"),
                ("status", None),
                ("test_input", None),
                ("test_options", None),
                ("eval_input", None),
                ("eval_options", None),
                ("test_cromwell_job_id", None),
                ("eval_cromwell_job_id", None),
                ("software_versions", None),
                ("created_before", None),
                ("created_after", None),
                ("created_by", None),
                ("finished_before", None),
                ("finished_after", None),
                ("sort", None),
                ("limit", None),
                ("offset", None),
                ("csv", True),
            ],
            "file_contents": b"randombytespresentingazip",
            "return": "Success!",
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).find_runs(...).thenReturn(None)
    # Mock up request response
    # Create a copy of the params so we can pass it to the mock and modify the original
    params_for_mock = copy.deepcopy(request.param["params"])
    params_for_mock[9] = (
        "software_versions",
        json.dumps(params_for_mock[9][1]) if params_for_mock[9][1] else None,
    )
    # If csv param is true, we have to create a temp file and include expected format in the function call
    if request.param["params"][18][1]:
        params_for_mock[18] = ("csv", "true")
        mockito.when(request_handler).find_runs(
            request.param["parent_entity"],
            request.param["parent_entity_id"],
            params_for_mock,
            expected_format=request_handler.ResponseFormat.BYTES,
        ).thenReturn(request.param["file_contents"])
        # Create a temp file that we'll write to
        request.param["file"] = tempfile.NamedTemporaryFile()
        request.param["params"][18] = ("csv", request.param["file"].name)
    else:
        params_for_mock[18] = ("csv", "false")
        mockito.when(request_handler).find_runs(
            request.param["parent_entity"],
            request.param["parent_entity_id"],
            params_for_mock,
        ).thenReturn(request.param["return"])
        request.param["params"][18] = ("csv", None)
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
        find_data["params"][16][1],
        find_data["params"][17][1],
        find_data["params"][18][1],
    )
    assert result == find_data["return"]
    # If the result is written to a zip, check that the zip has what we expect
    if "file_contents" in find_data:
        written_data = find_data["file"].read()
        assert written_data == find_data["file_contents"]


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
