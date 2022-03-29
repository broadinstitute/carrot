import json

import mockito
import pytest
from carrot_cli.rest import request_handler, software_builds


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
                    "status": "submitted",
                    "image_url": None,
                    "finished_at": None,
                    "build_job_id": "d041bcce-288f-4c7e-9f9d-b6af57ae2369",
                    "software_version_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "software_build_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No software_build found",
                    "status": 404,
                    "detail": "No software_build found with the specified ID",
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
        "software_builds", request.param["id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_by_id(find_by_id_data):
    result = software_builds.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("software_build_id", ""),
                ("software_version_id", ""),
                ("build_job_id", "d041bcce-288f-4c7e-9f9d-b6af57ae2369"),
                ("status", ""),
                ("image_url", ""),
                ("created_before", ""),
                ("created_after", ""),
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
                        "status": "submitted",
                        "image_url": None,
                        "finished_at": None,
                        "build_job_id": "d041bcce-288f-4c7e-9f9d-b6af57ae2369",
                        "software_version_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "software_build_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("software_build_id", ""),
                ("software_version_id", ""),
                ("build_job_id", "d041bcce-288f-4c7e-9f9d-b6af57ae2369"),
                ("status", ""),
                ("image_url", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("finished_before", ""),
                ("finished_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "title": "No software_builds found",
                    "status": 404,
                    "detail": "No software_builds found with the specified parameters",
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
    mockito.when(request_handler).find(
        "software_builds", request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    result = software_builds.find(
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
