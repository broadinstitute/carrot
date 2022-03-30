import json

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.rest import software_builds


@pytest.fixture(autouse=True)
def unstub():
    yield
    mockito.unstub()


@pytest.fixture(
    params=[
        {
            "args": [
                "software",
                "version",
                "build",
                "find_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            ],
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
            "args": [
                "software",
                "version",
                "build",
                "find_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            ],
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
    mockito.when(software_builds).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(software_builds).find_by_id(request.param["args"][4]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    runner = CliRunner()
    test_software_build = runner.invoke(carrot, find_by_id_data["args"])
    assert test_software_build.output == find_by_id_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "software",
                "version",
                "build",
                "find",
                "--software_build_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--software_version_id",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--build_job_id",
                "d041bcce-288f-4c7e-9f9d-b6af57ae2369",
                "--status",
                "succeeded",
                "--image_url",
                "example.com/image",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--finished_before",
                "2020-10-00T00:00:00.000000",
                "--finished_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(image_url)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "d041bcce-288f-4c7e-9f9d-b6af57ae2369",
                "succeeded",
                "example.com/image",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(image_url)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "status": "succeeded",
                        "image_url": "example.com/image",
                        "finished_at": "2020-09-16T18:58:06.371563",
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
            "args": [
                "software",
                "version",
                "build",
                "find",
                "--software_build_id",
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
                20,
                0,
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
    mockito.when(software_builds).find(...).thenReturn(None)
    # Mock up request response
    mockito.when(software_builds).find(
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
    test_software_build = runner.invoke(carrot, find_data["args"])
    assert test_software_build.output == find_data["return"] + "\n"
