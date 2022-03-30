import json

import mockito
import pytest
from carrot_cli.rest import pipelines, request_handler


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
                    "description": "This pipeline will save Etheria",
                    "name": "Sword of Protection Pipeline",
                    "pipeline_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No pipeline found",
                    "status": 404,
                    "detail": "No pipeline found with the specified ID",
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
        "pipelines", request.param["id"]
    ).thenReturn(request.param["return"])
    return request.param


def test_find_by_id(find_by_id_data):
    result = pipelines.find_by_id(find_by_id_data["id"])
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "params": [
                ("pipeline_id", ""),
                ("name", "Queen of Bright Moon Pipeline"),
                ("description", ""),
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
                        "description": "This pipeline leads the Rebellion",
                        "name": "Queen of Bright Moon Pipeline",
                        "pipeline_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("pipeline_id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"),
                ("name", ""),
                ("description", ""),
                ("created_by", ""),
                ("created_before", ""),
                ("created_after", ""),
                ("sort", ""),
                ("limit", ""),
                ("offset", ""),
            ],
            "return": json.dumps(
                {
                    "title": "No pipelines found",
                    "status": 404,
                    "detail": "No pipelines found with the specified parameters",
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
    mockito.when(request_handler).find("pipelines", request.param["params"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find(find_data):
    result = pipelines.find(
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


@pytest.fixture(
    params=[
        {
            "params": [
                ("name", "Horde Emperor Pipeline"),
                ("description", "This pipeline rules the known universe"),
                ("created_by", "hordeprime@example.com"),
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "hordeprime@example.com",
                    "description": "This pipeline rules the known universe",
                    "name": "Horde Emperor Pipeline",
                    "pipeline_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "params": [
                ("name", "Horde Emperor Pipeline"),
                ("description", "This pipeline rules the known universe"),
                ("created_by", "hordeprime@example.com"),
            ],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to insert new pipeline",
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
    mockito.when(request_handler).create(
        "pipelines", request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_create(create_data):
    result = pipelines.create(
        create_data["params"][0][1],
        create_data["params"][1][1],
        create_data["params"][2][1],
    )
    assert result == create_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
            "params": [
                ("name", "Catra Pipeline"),
                (
                    "description",
                    "This pipeline is trying to learn to process anger better",
                ),
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-16T18:48:08.371563",
                    "created_by": "catra@example.com",
                    "description": "This pipeline is trying to learn to process anger better",
                    "name": "Catra Pipeline",
                    "pipeline_id": "bd132568-06fe-4b1a-9e96-47d4f36bf819",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "params": [("name", "Angella Pipeline"), ("description", "")],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to update new pipeline",
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
        "pipelines", request.param["id"], request.param["params"]
    ).thenReturn(request.param["return"])
    return request.param


def test_update(update_data):
    result = pipelines.update(
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
                    "title": "No pipeline found",
                    "status": 404,
                    "detail": "No pipeline found with the specified ID",
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
    mockito.when(request_handler).delete("pipelines", request.param["id"]).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    result = pipelines.delete(delete_data["id"])
    assert result == delete_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
            "email": "bow@example.com",
            "return": json.dumps(
                {
                    "subscription_id": "361b3b95-4a6e-40d9-bd98-f92b2959864e",
                    "entity_type": "pipeline",
                    "entity_id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
                    "email": "bow@example.com",
                    "created_at": "2020-09-23T19:41:46.839880",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "email": "huntara@example.com",
            "return": json.dumps(
                {
                    "title": "No pipeline found",
                    "status": 404,
                    "detail": "No pipeline found with the specified ID",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def subscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).update(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).subscribe(
        "pipelines", request.param["id"], request.param["email"]
    ).thenReturn(request.param["return"])
    return request.param


def test_subscribe(subscribe_data):
    result = pipelines.subscribe(
        subscribe_data["id"],
        subscribe_data["email"],
    )
    assert result == subscribe_data["return"]


@pytest.fixture(
    params=[
        {
            "id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
            "email": "mermista@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
        {
            "id": "98536487-06fe-4b1a-9e96-47d4f36bf819",
            "email": "castaspella@example.com",
            "return": json.dumps(
                {
                    "title": "No subscription found",
                    "status": 404,
                    "detail": "No subscription found for the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def unsubscribe_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).update(...).thenReturn(None)
    # Mock up request response
    mockito.when(request_handler).unsubscribe(
        "pipelines", request.param["id"], request.param["email"]
    ).thenReturn(request.param["return"])
    return request.param


def test_unsubscribe(unsubscribe_data):
    result = pipelines.unsubscribe(
        unsubscribe_data["id"],
        unsubscribe_data["email"],
    )
    assert result == unsubscribe_data["return"]
