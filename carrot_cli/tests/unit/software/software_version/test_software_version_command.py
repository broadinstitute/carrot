import json

from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.rest import software, software_versions


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
                "find_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:06.371563",
                    "commit": "ca82a6dff817ec66f44342007202690a93763949",
                    "software_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "software_version_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "args": [
                "software",
                "version",
                "find_by_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "return": json.dumps(
                {
                    "title": "No software_version found",
                    "status": 404,
                    "detail": "No software_version found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(software_versions).find_by_id(...).thenReturn(None)
    # Mock up request response
    mockito.when(software_versions).find_by_id(request.param["args"][3]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    runner = CliRunner()
    test_software_version = runner.invoke(carrot, find_by_id_data["args"])
    assert test_software_version.output == find_by_id_data["return"] + "\n"


@pytest.fixture(
    params=[
        {
            "args": [
                "software",
                "version",
                "find",
                "--software_version_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--software_id",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "--commit",
                "ca82a6dff817ec66f44342007202690a93763949",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(commit)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "ca82a6dff817ec66f44342007202690a93763949",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(commit)",
                1,
                0,
            ],
            "return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "commit": "ca82a6dff817ec66f44342007202690a93763949",
                        "software_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "software_version_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "find",
                "--software_version_id",
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "--software",
                "New Sword of Protection software",
                "--commit",
                "ca82a6dff817ec66f44342007202690a93763949",
                "--created_before",
                "2020-10-00T00:00:00.000000",
                "--created_after",
                "2020-09-00T00:00:00.000000",
                "--sort",
                "asc(commit)",
                "--limit",
                1,
                "--offset",
                0,
            ],
            "params": [
                "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                "ca82a6dff817ec66f44342007202690a93763949",
                "2020-10-00T00:00:00.000000",
                "2020-09-00T00:00:00.000000",
                "asc(commit)",
                1,
                0,
            ],
            "from_name": {
                "name": "New Sword of Protection software",
                "return": json.dumps(
                    [
                        {
                            "created_at": "2020-09-16T18:48:06.371563",
                            "created_by": "adora@example.com",
                            "repository_url": "example.com/repo.git",
                            "description": "This new software replaced the broken one",
                            "name": "New Sword of Protection software",
                            "software_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
                        "commit": "ca82a6dff817ec66f44342007202690a93763949",
                        "software_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                        "software_version_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
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
                "find",
                "--software_version_id",
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
            ],
            "params": [
                "986325ba-06fe-4b1a-9e96-47d4f36bf819",
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
                    "title": "No software_versions found",
                    "status": 404,
                    "detail": "No software_versions found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(software_versions).find(...).thenReturn(None)
    # If there's a value for from_name, set the return value for trying to retrieve the existing
    # record
    if "from_name" in request.param:
        mockito.when(software).find(
            name=request.param["from_name"]["name"],
            limit=2
        ).thenReturn(request.param["from_name"]["return"])
    # Mock up request response
    mockito.when(software_versions).find(
        request.param["params"][0],
        request.param["params"][1],
        request.param["params"][2],
        request.param["params"][3],
        request.param["params"][4],
        request.param["params"][5],
        request.param["params"][6],
        request.param["params"][7],
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    runner = CliRunner()
    test_software_version = runner.invoke(carrot, find_data["args"])
    assert test_software_version.output == find_data["return"] + "\n"
