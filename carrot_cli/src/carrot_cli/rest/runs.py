import json
import logging

from . import request_handler
from .. import file_util

LOGGER = logging.getLogger(__name__)


def find_by_id(run_id, csv=None):
    """
    Submits a request to CARROT's runs find_by_id mapping. If csv is specified as true, the request
    will include a query param called 'csv' set to true
    """
    if csv is not None:
        # Write to file
        csv_data = request_handler.find_by_id("runs", run_id, params=[("csv", "true")], expected_format=request_handler.ResponseFormat.BYTES)
        file_util.write_data_to_file(csv_data, csv)
        return "Success!"
    else:
        return request_handler.find_by_id("runs", run_id)


def find(
    parent_entity=None,
    parent_entity_id=None,
    run_group_id=None,
    name=None,
    status=None,
    test_input=None,
    test_options=None,
    eval_input=None,
    eval_options=None,
    test_cromwell_job_id=None,
    eval_cromwell_job_id=None,
    software_versions=None,
    created_before=None,
    created_after=None,
    created_by=None,
    finished_before=None,
    finished_after=None,
    sort=None,
    limit=None,
    offset=None,
    csv=None
):
    """
    Submits a request to CARROT's find runs mapping for the specified parent_entity (pipeline,
    template, or test) with the specified id, filtering by the other parameters
    """
    # Create parameter list
    params = [
        ("run_group_id", run_group_id),
        ("name", name),
        ("status", status),
        ("test_input", test_input),
        ("test_options", test_options),
        ("eval_input", eval_input),
        ("eval_options", eval_options),
        ("test_cromwell_job_id", test_cromwell_job_id),
        ("eval_cromwell_job_id", eval_cromwell_job_id),
        ("software_versions", json.dumps(software_versions) if software_versions else None),
        ("created_before", created_before),
        ("created_after", created_after),
        ("created_by", created_by),
        ("finished_before", finished_before),
        ("finished_after", finished_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
        ("csv", str(csv is not None).lower())
    ]
    # If csv is true, we want to specify that we're expecting the result as a file (bytes)
    if csv is not None:
        # Write to file
        csv_data = request_handler.find_runs(
            parent_entity, parent_entity_id, params, expected_format=request_handler.ResponseFormat.BYTES
        )
        file_util.write_data_to_file(csv_data, csv)
        return "Success!"
    return request_handler.find_runs(parent_entity, parent_entity_id, params)


def delete(run_id):
    """Submits a request to CARROT's runs delete mapping"""
    return request_handler.delete("runs", run_id)
