import json
import logging

import requests

import mockito
import pytest
from carrot_cli.config import manager as config
from carrot_cli.rest import request_handler


@pytest.fixture(autouse=True)
def unstub():
    yield
    mockito.unstub()


@pytest.fixture(
    params=[
        {
            "entity": "pipelines",
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
            "entity": "templates",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "created_at": "2020-09-02T13:41:44.217522",
                    "created_by": "catra@example.com",
                    "description": "This template has problems with misdirected aggression",
                    "test_wdl": "example.com/horde_test.wdl",
                    "eval_wdl": "example.com/horde_eval.wdl",
                    "name": "Catra template",
                    "pipeline_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "58723b05-6060-4444-9f1b-394aff691cce",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_by_id_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s" % (
        "example.com",
        request.param["entity"],
        request.param["id"],
    )
    mockito.when(request_handler).send_request("GET", address).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_by_id(find_by_id_data):
    result = request_handler.find_by_id(
        find_by_id_data["entity"], find_by_id_data["id"]
    )
    assert result == find_by_id_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "pipelines",
            "params": [("name", "Queen of Bright Moon Pipeline")],
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
            "entity": "pipelines",
            "params": [("id", "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8"), ("name", "")],
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s" % ("example.com", request.param["entity"])
    # Get params filtered to remove empty ones since the empty ones won't be passed to request
    params = list(filter(lambda param: param[1] != "", request.param["params"]))
    mockito.when(request_handler).send_request(
        "GET", address, params=params
    ).thenReturn(request.param["return"])
    return request.param


def test_find(find_data):
    result = request_handler.find(find_data["entity"], find_data["params"])
    assert result == find_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "pipelines",
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
            "entity": "templates",
            "params": [],
            "return": "Received response with status 500 and empty body",
        },
    ]
)
def create_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s" % ("example.com", request.param["entity"])
    # Get params converted to dict
    params = dict(request.param["params"])
    mockito.when(request_handler).send_request("POST", address, json=params).thenReturn(
        request.param["return"]
    )
    return request.param


def test_create(create_data):
    result = request_handler.create(create_data["entity"], create_data["params"])
    assert result == create_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "templates",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "params": [
                ("description", "This template is working on her abandonment issues")
            ],
            "return": json.dumps(
                {
                    "created_at": "2020-09-02T13:41:44.217522",
                    "created_by": "catra@example.com",
                    "description": "This template is working on her abandonment issues",
                    "test_wdl": "example.com/horde_test.wdl",
                    "eval_wdl": "example.com/horde_eval.wdl",
                    "name": "Catra template",
                    "pipeline_id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
                    "template_id": "58723b05-6060-4444-9f1b-394aff691cce",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity": "pipelines",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "params": [],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to update pipeline",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def update_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s" % (
        "example.com",
        request.param["entity"],
        request.param["id"],
    )
    # Get params converted to dict
    params = dict(request.param["params"])
    mockito.when(request_handler).send_request("PUT", address, json=params).thenReturn(
        request.param["return"]
    )
    return request.param


def test_update(update_data):
    result = request_handler.update(
        update_data["entity"], update_data["id"], update_data["params"]
    )
    assert result == update_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "pipelines",
            "id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "entity": "templates",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "return": json.dumps(
                {
                    "title": "No template found",
                    "status": 404,
                    "detail": "No template found for the specified id",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def delete_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s" % (
        "example.com",
        request.param["entity"],
        request.param["id"],
    )
    mockito.when(request_handler).send_request("DELETE", address).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete(delete_data):
    result = request_handler.delete(delete_data["entity"], delete_data["id"])
    assert result == delete_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "pipelines",
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
            "entity": "pipelines",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/subscriptions" % (
        "example.com",
        request.param["entity"],
        request.param["id"],
    )
    # Make request body with email
    body = {"email": request.param["email"]}
    mockito.when(request_handler).send_request("POST", address, json=body).thenReturn(
        request.param["return"]
    )
    return request.param


def test_subscribe(subscribe_data):
    result = request_handler.subscribe(
        subscribe_data["entity"], subscribe_data["id"], subscribe_data["email"]
    )
    assert result == subscribe_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "pipelines",
            "id": "047e27ad-2890-4372-b2cb-dfec57347eb9",
            "email": "mermista@example.com",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row(s)"}, indent=4, sort_keys=True
            ),
        },
        {
            "entity": "pipelines",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/subscriptions" % (
        "example.com",
        request.param["entity"],
        request.param["id"],
    )
    # Make request parmas with email
    params = [("email", request.param["email"])]
    mockito.when(request_handler).send_request(
        "DELETE", address, params=params
    ).thenReturn(request.param["return"])
    return request.param


def test_unsubscribe(unsubscribe_data):
    result = request_handler.unsubscribe(
        unsubscribe_data["entity"], unsubscribe_data["id"], unsubscribe_data["email"]
    )
    assert result == unsubscribe_data["return"]


@pytest.fixture(
    params=[
        {
            "test_id": "c97c25e5-4adf-4db7-8f64-19af34d84ef8",
            "params": [
                ("name", 'Did someone say "Swift Wind"?'),
                ("test_input", {"species": "Alicorn"}),
                ("created_by", "swiftwind@example.com"),
            ],
            "return": json.dumps(
                {
                    "run_id": "69717609-0e95-4c9c-965c-1ea40a2cf44f",
                    "test_id": "c97c25e5-4adf-4db7-8f64-19af34d84ef8",
                    "name": 'Did someone say "Swift Wind"?',
                    "status": "submitted",
                    "test_input": {"species": "Alicorn"},
                    "eval_input": {"home": "Bright Moon"},
                    "cromwell_job_id": "f95ff110-88bd-4473-a5e1-dc7d5fc48d3a",
                    "created_at": "2020-09-24T15:50:49.641333",
                    "created_by": "swiftwind@example.com",
                    "finished_at": None,
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "test_id": "235abc9d-1357-4db7-8f64-19af34d84ef8",
            "params": [
                ("name", "Meditation Run"),
                ("test_input", {"occupation": "Princess"}),
                ("created_by", "perfuma@example.com"),
            ],
            "return": json.dumps(
                {
                    "title": "Server error",
                    "status": 500,
                    "detail": "Error while attempting to query the database: NotFound",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def run_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/tests/%s/runs" % (
        "example.com",
        request.param["test_id"],
    )
    # Get params converted to dict
    params = dict(request.param["params"])
    mockito.when(request_handler).send_request("POST", address, json=params).thenReturn(
        request.param["return"]
    )
    return request.param


def test_run(run_data):
    result = request_handler.run(run_data["test_id"], run_data["params"])
    assert result == run_data["return"]


@pytest.fixture(
    params=[
        {
            "entity": "tests",
            "id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "params": [("name", "Entrapta Test Run")],
            "return": json.dumps(
                [
                    {
                        "run_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
                        "test_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
                        "name": "Entrapta Test Run",
                        "status": "succeeded",
                        "test_input": {"in_tech": "First Ones"},
                        "eval_input": {
                            "in_type": "Spaceship",
                        },
                        "cromwell_job_id": "d041bcce-288f-4c7e-9f9d-b6af57ae2369",
                        "created_at": "2020-09-10T13:43:13.657729",
                        "created_by": "entrapta@example.com",
                        "finished_at": "2020-09-10T14:03:27.658",
                        "results": {"ShipName": "Darla"},
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity": "templates",
            "id": "3d1bfbab-d9ec-46c7-aa8e-9c1d1808f2b8",
            "params": [("id", ""), ("name", "Mara's Test Run")],
            "return": json.dumps(
                {
                    "title": "No run found",
                    "status": 404,
                    "detail": "No runs found with the specified parameters",
                },
                indent=4,
                sort_keys=True,
            ),
        },
    ]
)
def find_runs_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/runs" % (
        "example.com",
        request.param["entity"],
        request.param["id"],
    )
    # Get params filtered to remove empty ones since the empty ones won't be passed to request
    params = list(filter(lambda param: param[1] != "", request.param["params"]))
    mockito.when(request_handler).send_request(
        "GET", address, params=params
    ).thenReturn(request.param["return"])
    return request.param


def test_find_runs(find_runs_data):
    result = request_handler.find_runs(
        find_runs_data["entity"], find_runs_data["id"], find_runs_data["params"]
    )
    assert result == find_runs_data["return"]


@pytest.fixture(
    params=[
        {
            "entity1": "templates",
            "entity1_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "entity2": "results",
            "entity2_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
            "params": [
                ("result_key", "out_horde_tanks"),
                ("created_by", "rogelio@example.com"),
            ],
            "return": json.dumps(
                {
                    "template_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
                    "result_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
                    "result_key": "out_horde_tanks",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity1": "templates",
            "entity1_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "entity2": "results",
            "entity2_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
            "params": [
                ("result_key", "out_force_captain"),
                ("created_by", "kyle@example.com"),
            ],
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/%s/%s" % (
        "example.com",
        request.param["entity1"],
        request.param["entity1_id"],
        request.param["entity2"],
        request.param["entity2_id"],
    )
    # Get params converted to dict
    params = dict(request.param["params"])
    mockito.when(request_handler).send_request(
        "POST", address, json=params, params=None
    ).thenReturn(request.param["return"])
    return request.param


def test_create_map(create_map_data):
    result = request_handler.create_map(
        create_map_data["entity1"],
        create_map_data["entity1_id"],
        create_map_data["entity2"],
        create_map_data["entity2_id"],
        create_map_data["params"],
    )
    assert result == create_map_data["return"]


@pytest.fixture(
    params=[
        {
            "entity1": "templates",
            "entity1_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "entity2": "results",
            "entity2_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
            "return": json.dumps(
                {
                    "template_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
                    "result_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
                    "result_key": "out_horde_tanks",
                    "created_at": "2020-09-24T19:07:59.311462",
                    "created_by": "rogelio@example.com",
                },
                indent=4,
                sort_keys=True,
            ),
        },
        {
            "entity1": "templates",
            "entity1_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "entity2": "results",
            "entity2_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/%s/%s" % (
        "example.com",
        request.param["entity1"],
        request.param["entity1_id"],
        request.param["entity2"],
        request.param["entity2_id"],
    )
    mockito.when(request_handler).send_request("GET", address).thenReturn(
        request.param["return"]
    )
    return request.param


def test_find_map_by_ids(find_map_by_ids_data):
    result = request_handler.find_map_by_ids(
        find_map_by_ids_data["entity1"],
        find_map_by_ids_data["entity1_id"],
        find_map_by_ids_data["entity2"],
        find_map_by_ids_data["entity2_id"],
    )
    assert result == find_map_by_ids_data["return"]


@pytest.fixture(
    params=[
        {
            "entity1": "templates",
            "entity1_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "entity2": "results",
            "entity2_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
            "return": json.dumps(
                {"message": "Successfully deleted 1 row"}, indent=4, sort_keys=True
            ),
        },
        {
            "entity1": "templates",
            "entity1_id": "5fad47be-0d23-4679-8d8c-deff717d5419",
            "entity2": "results",
            "entity2_id": "8ff51b0a-cdbf-409f-9e8b-888524ae9c1a",
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/%s/%s" % (
        "example.com",
        request.param["entity1"],
        request.param["entity1_id"],
        request.param["entity2"],
        request.param["entity2_id"],
    )
    mockito.when(request_handler).send_request("DELETE", address).thenReturn(
        request.param["return"]
    )
    return request.param


def test_delete_map_by_ids(delete_map_by_ids_data):
    result = request_handler.delete_map_by_ids(
        delete_map_by_ids_data["entity1"],
        delete_map_by_ids_data["entity1_id"],
        delete_map_by_ids_data["entity2"],
        delete_map_by_ids_data["entity2_id"],
    )
    assert result == delete_map_by_ids_data["return"]


@pytest.fixture(
    params=[
        {
            "entity1": "templates",
            "entity1_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "entity2": "results",
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
            "entity1": "templates",
            "entity1_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
            "entity2": "results",
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
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Mock up request response
    address = "http://%s/api/v1/%s/%s/%s" % (
        "example.com",
        request.param["entity1"],
        request.param["entity1_id"],
        request.param["entity2"],
    )
    # Get params filtered to remove empty ones since the empty ones won't be passed to request
    params = list(filter(lambda param: param[1] != "", request.param["params"]))
    mockito.when(request_handler).send_request(
        "GET", address, params=params
    ).thenReturn(request.param["return"])
    return request.param


def test_find_maps(find_maps_data):
    result = request_handler.find_maps(
        find_maps_data["entity1"],
        find_maps_data["entity1_id"],
        find_maps_data["entity2"],
        find_maps_data["params"],
    )
    assert result == find_maps_data["return"]


@pytest.fixture(
    params=[
        {
            "exception": requests.ConnectionError,
            "return": "Encountered a connection error. Enable verbose logging (-v) for more info",
        },
        {
            "exception": requests.URLRequired,
            "return": "Invalid URL. Enable verbose logging (-v) for more info",
        },
        {
            "exception": requests.Timeout,
            "return": "Request timed out. Enable verbose logging (-v) for more info",
        },
        {
            "exception": requests.TooManyRedirects,
            "return": "Too many redirects. Enable verbose logging (-v) for more info",
        },
        {
            "status_code": 400,
            "text": "",
            "return": json.dumps({"Body": "", "Status": 400}, indent=4, sort_keys=True),
        },
        {
            "status_code": 200,
            "text": json.dumps({"test_id": "123456789", "name": "test_name"}),
            "return": json.dumps(
                {"name": "test_name", "test_id": "123456789"}, indent=4, sort_keys=True
            ),
        },
    ]
)
def send_request_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(requests).request(...).thenReturn(None)
    # Params to pass to make sure it processes them properly
    params = [("sort", "asc(name)")]
    json_body = {"test", "test"}
    # For exceptions, if we get a request, raise the exception
    if "exception" in request.param:
        mockito.when(requests).request(
            "POST", "http://example.com/api/v1/pipelines", params=params, json=json_body, data=None, files=None
        ).thenRaise(request.param["exception"])
    # Otherwise, set it to return the specified response
    else:
        result = {
            "status_code": request.param["status_code"],
            "text": request.param["text"],
        }
        response = mockito.mock(result, spec=requests.Response)
        if request.param["text"] != "":
            mockito.when(response).json().thenReturn(json.loads(request.param["text"]))
        mockito.when(requests).request(
            "POST", "http://example.com/api/v1/pipelines", params=params, json=json_body, data=None, files=None
        ).thenReturn(response)

    return request.param["return"]


def test_send_request(send_request_data):
    params = [("sort", "asc(name)")]
    body = {"test", "test"}
    # Send request
    response = request_handler.send_request(
        "POST", "http://example.com/api/v1/pipelines", params=params, json=body
    )
    # Check that we got the expected error message
    assert response == send_request_data


@pytest.fixture(
    params=[
        {
            "files": {
                "test_wdl_file": "tests/data/test.wdl",
                "eval_wdl_file": "tests/data/eval.wdl",
                "test_wdl_dependencies_file": "tests/data/test_dep.zip",
                "eval_wdl_dependencies_file": "tests/data/eval_dep.zip"
            },
            "success": True
        },
        {
            "files": {
                "test_wdl_file": "tests/data/not_a_real.wdl",
                "eval_wdl_file": "tests/data/eval.wdl",
                "test_wdl_dependencies_file": "tests/data/test_dep.zip",
                "eval_wdl_dependencies_file": "tests/data/eval_dep.zip"
            },
            "success": False,
            "logging": "Failed to open test_wdl_file file with path tests/data/not_a_real.wdl"
        }
    ]
)
def process_file_dict_data(request):
    return request.param


def test_process_file_dict(process_file_dict_data, caplog):
    if process_file_dict_data["success"]:
        result = request_handler.__process_file_dict(process_file_dict_data["files"])
        for key, value in result.items():
            assert value[0] == process_file_dict_data["files"][key].rsplit("/", 1)[1]
            assert value[1].name == process_file_dict_data["files"][key]
    else:
        with pytest.raises(IOError):
            request_handler.__process_file_dict(process_file_dict_data["files"])
        assert process_file_dict_data["logging"] in caplog.text
