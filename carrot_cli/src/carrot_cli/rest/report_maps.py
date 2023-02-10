import json
import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def create_map(entity, entity_id, report_id, created_by, delete_failed):
    """Submits a request to CARROT's report_map create mapping"""
    if delete_failed:
        delete_failed = "true"
    else:
        delete_failed = "false"
    return request_handler.create_map(
        entity,
        entity_id,
        "reports",
        report_id,
        [("created_by", created_by)],
        [("delete_failed", delete_failed)],
    )

def create_map_from_run_query(
    report_id,
    created_by,
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
    run_created_by=None,
    finished_before=None,
    finished_after=None,
    sort=None,
    limit=None,
    offset=None,
):
    """Submits a request to CARROT's report_map create from run query mapping"""
    # Create parameter list
    query_params = [
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
        ("created_by", run_created_by),
        ("finished_before", finished_before),
        ("finished_after", finished_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    body_params = [
        ("created_by", created_by)
    ]
    return request_handler.create_report_map_from_run_query(
        report_id, parent_entity, parent_entity_id, body_params, query_params
    )


def find_maps(
    entity,
    entity_id,
    report_id,
    status,
    cromwell_job_id,
    results,
    created_before,
    created_after,
    created_by,
    finished_before,
    finished_after,
    sort,
    limit,
    offset,
):
    """Submits a request to CARROT's report_map find mapping"""
    # Create parameter list
    params = [
        ("report_id", report_id),
        ("status", status),
        ("cromwell_job_id", cromwell_job_id),
        ("results", results),
        ("created_before", created_before),
        ("created_after", created_after),
        ("created_by", created_by),
        ("finished_before", finished_before),
        ("finished_after", finished_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find_maps(entity, entity_id, "reports", params)


def find_map_by_ids(entity, entity_id, report_id):
    """Submits a request to CARROT's report_map find_by_id mapping"""
    return request_handler.find_map_by_ids(entity, entity_id, "reports", report_id)


def delete_map_by_ids(entity, entity_id, report_id):
    """Submits a request to CARROT's report_map delete mapping"""
    return request_handler.delete_map_by_ids(entity, entity_id, "reports", report_id)
