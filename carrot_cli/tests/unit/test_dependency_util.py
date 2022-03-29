import json
import logging

import mockito
import pytest

from carrot_cli import dependency_util
from carrot_cli.rest import pipelines

@pytest.fixture(
    params=[
        {
            "id_or_name": "Test name",
            "module": pipelines,
            "id_key": "pipeline_id",
            "entity_name": "pipeline",
            "request_return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "created_by": "adora@example.com",
                        "description": "This is the old description for this pipeline",
                        "name": "Sword of Protection pipeline",
                        "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
            "return": "550e8400-e29b-41d4-a716-446655440000"
        },
        {
            "id_or_name": "550e8400-e29b-41d4-a716-446655440000",
            "module": pipelines,
            "id_key": "pipeline_id",
            "entity_name": "pipeline",
            "request_return": None,
            "return": "550e8400-e29b-41d4-a716-446655440000"
        },
        {
            "id_or_name": "Test name",
            "module": pipelines,
            "id_key": "pipeline_id",
            "entity_name": "pipeline",
            "request_return": json.dumps(
                {
                    "title": "No pipeline found",
                    "status": 404,
                    "detail": "No pipelines found with the specified parameters"
                },
                indent=4,
                sort_keys=True,
            ),
            "logging": "Encountered an error processing value for pipeline: " +
               json.dumps(
                    {
                        "title": "No pipeline found",
                        "status": 404,
                        "detail": "No pipelines found with the specified parameters"
                    },
                    indent=4,
                    sort_keys=True,
                ),
        },
        {
            "id_or_name": "Test name",
            "module": pipelines,
            "id_key": "pipeline_id",
            "entity_name": "pipeline",
            "request_return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "created_by": "adora@example.com",
                        "description": "This is the old description for this pipeline",
                        "name": "Sword of Protection pipeline",
                        "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                    },
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "created_by": "adora@example.com",
                        "description": "This pipeline is one we weren't looking for",
                        "name": "Some other pipeline",
                        "pipeline_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
            "logging": "Encountered an error processing value for pipeline: Attempt to retrieve record by name produced "
                       "unexpected result: " +
                       json.dumps(
                            [
                                {
                                    "created_at": "2020-09-16T18:48:06.371563",
                                    "created_by": "adora@example.com",
                                    "description": "This is the old description for this pipeline",
                                    "name": "Sword of Protection pipeline",
                                    "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                                },
                                {
                                    "created_at": "2020-09-16T18:48:06.371563",
                                    "created_by": "adora@example.com",
                                    "description": "This pipeline is one we weren't looking for",
                                    "name": "Some other pipeline",
                                    "pipeline_id": "cd987859-06fe-4b1a-9e96-47d4f36bf819",
                                }
                            ],
                            indent=4,
                            sort_keys=True,
                       ),
        },
        {
            "id_or_name": "Test name",
            "module": pipelines,
            "id_key": "pipeline_id",
            "entity_name": "pipeline",
            "request_return": json.dumps([], indent=4, sort_keys=True),
            "logging": "Encountered an error processing value for pipeline: Attempt to retrieve record by name produced "
                       "unexpected result: " + json.dumps([], indent=4, sort_keys=True),
        },
        {
            "id_or_name": "Test name",
            "module": pipelines,
            "id_key": "template_id",
            "entity_name": "pipeline",
            "request_return": json.dumps(
                [
                    {
                        "created_at": "2020-09-16T18:48:06.371563",
                        "created_by": "adora@example.com",
                        "description": "This is the old description for this pipeline",
                        "name": "Sword of Protection pipeline",
                        "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                    }
                ],
                indent=4,
                sort_keys=True,
            ),
            "logging": "Encountered an error processing value for pipeline: Attempt to retrieve template_id by name "
                       "failed with record: " +
                       json.dumps(
                        [
                            {
                                "created_at": "2020-09-16T18:48:06.371563",
                                "created_by": "adora@example.com",
                                "description": "This is the old description for this pipeline",
                                "name": "Sword of Protection pipeline",
                                "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                            }
                        ],
                        indent=4,
                        sort_keys=True,
                    ),
        },
        {
            "id_or_name": "Test name",
            "module": pipelines,
            "id_key": "pipeline_id",
            "entity_name": "pipeline",
            "request_return": "The Horde prevented your request from being processed.",
            "logging": "Encountered an error processing value for pipeline: The Horde prevented your request from being "
                       "processed.",
        },
    ]
)
def get_id_from_id_or_name_and_handle_error_data(request):
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(pipelines).find(...).thenReturn(None)
    # Make sure our pipeline request returns the value we want to test
    mockito.when(pipelines).find(
        name=request.param["id_or_name"],
        limit=2
    ).thenReturn(request.param["request_return"])
    return request.param

def test_get_id_from_id_or_name_and_handle_error(get_id_from_id_or_name_and_handle_error_data, caplog):
    # If there's logging, that means we're testing an error, so we want to makre sure it's logging
    # the right thing
    if "logging" in get_id_from_id_or_name_and_handle_error_data:
        with pytest.raises(SystemExit):
            dependency_util.get_id_from_id_or_name_and_handle_error(
                get_id_from_id_or_name_and_handle_error_data["id_or_name"],
                get_id_from_id_or_name_and_handle_error_data["module"],
                get_id_from_id_or_name_and_handle_error_data["id_key"],
                get_id_from_id_or_name_and_handle_error_data["entity_name"]
            )
        assert get_id_from_id_or_name_and_handle_error_data["logging"] in caplog.text
    # Otherwise, just check to make sure it returned the expected value
    else:
        result = dependency_util.get_id_from_id_or_name_and_handle_error(
            get_id_from_id_or_name_and_handle_error_data["id_or_name"],
            get_id_from_id_or_name_and_handle_error_data["module"],
            get_id_from_id_or_name_and_handle_error_data["id_key"],
            get_id_from_id_or_name_and_handle_error_data["entity_name"]
        )
        assert result == get_id_from_id_or_name_and_handle_error_data["return"]
