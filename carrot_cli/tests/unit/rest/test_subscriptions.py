import json

import mockito
import pytest
from carrot_cli.rest import request_handler, subscriptions


@pytest.fixture(autouse=True)
def unstub():
    yield
    mockito.unstub()


@pytest.fixture(
    params=[
        {
            "id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
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
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No subscription found",
                    "status": 404,
                    "detail": "No subscription found with the specified ID",
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
        "subscriptions", request.param["id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_by_id(find_by_id_data):
    result = subscriptions.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("subscription_id", ""),
                ("entity_type", "template"),
                ("entity_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("created_before", ""),
                ("created_after", ""),
                ("email", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                [
                    {
                        "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                        "entity_type": "template",
                        "entity_id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
                        "email": "scorpia@example.com",
                        "created_at": "2020-09-23T19:41:46.839880",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("subscription_id", ""),
                ("entity_type", "test"),
                ("entity_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("created_before", ""),
                ("created_after", ""),
                ("email", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "title": "No subscriptions found",
                    "status": 404,
                    "detail": "No subscriptions found with the specified parameters",
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
        "subscriptions", request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    result = subscriptions.find(
        find_data["params"][0][1],
        find_data["params"][1][1],
        find_data["params"][2][1],
        find_data["params"][3][1],
        find_data["params"][4][1],
        find_data["params"][5][1],
        find_data["params"][6][1],
        find_data["params"][7][1],
        find_data["params"][8][1],
    )
    assert result == find_data["return"]
