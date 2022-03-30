import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(run_id):
    """Submits a request to CARROT's runs find_by_id mapping"""
    return request_handler.find_by_id("runs", run_id)


def find(
    parent_entity="",
    parent_entity_id="",
    name="",
    status="",
    test_input="",
    test_options="",
    eval_input="",
    eval_options="",
    test_cromwell_job_id="",
    eval_cromwell_job_id="",
    created_before="",
    created_after="",
    created_by="",
    finished_before="",
    finished_after="",
    sort="",
    limit="",
    offset="",
):
    """
    Submits a request to CARROT's find runs mapping for the specfied parent_entity (pipeline,
    template, or test) with the specified id, filtering by the other parameters
    """
    # Create parameter list
    params = [
        ("name", name),
        ("status", status),
        ("test_input", test_input),
        ("test_options", test_options),
        ("eval_input", eval_input),
        ("eval_options", eval_options),
        ("test_cromwell_job_id", test_cromwell_job_id),
        ("eval_cromwell_job_id", eval_cromwell_job_id),
        ("created_before", created_before),
        ("created_after", created_after),
        ("created_by", created_by),
        ("finished_before", finished_before),
        ("finished_after", finished_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find_runs(parent_entity, parent_entity_id, params)


def delete(run_id):
    """Submits a request to CARROT's runs delete mapping"""
    return request_handler.delete("runs", run_id)
