import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def create_map(entity, run_id, report_id, created_by, delete_failed):
    """Submits a request to CARROT's report_map create mapping"""
    if delete_failed:
        delete_failed = "true"
    else:
        delete_failed = "false"
    return request_handler.create_map(
        entity,
        run_id,
        "reports",
        report_id,
        [("created_by", created_by)],
        [("delete_failed", delete_failed)],
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
