import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def create_map(template_id, report_id, report_trigger, created_by):
    """Submits a request to CARROT's template_report create mapping"""
    return request_handler.create_map_with_target(
        "templates",
        template_id,
        "reports",
        report_id,
        report_trigger,
        [("created_by", created_by)],
    )


def find_maps(
    template_id,
    report_id,
    report_trigger,
    created_before,
    created_after,
    created_by,
    sort,
    limit,
    offset,
):
    """Submits a request to CARROT's template_report find mapping"""
    # Create parameter list
    params = [
        ("report_id", report_id),
        ("report_trigger", report_trigger),
        ("created_before", created_before),
        ("created_after", created_after),
        ("created_by", created_by),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find_maps("templates", template_id, "reports", params)


def find_map_by_ids(template_id, report_id, report_trigger):
    """Submits a request to CARROT's template_report find_by_id mapping"""
    return request_handler.find_map_by_ids_and_target(
        "templates", template_id, "reports", report_id, report_trigger
    )


def delete_map_by_ids(template_id, report_id, report_trigger):
    """Submits a request to CARROT's template_report delete mapping"""
    return request_handler.delete_map_by_ids_and_target(
        "templates", template_id, "reports", report_id, report_trigger
    )
