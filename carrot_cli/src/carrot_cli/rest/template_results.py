import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def create_map(template_id, result_id, result_key, created_by):
    """Submits a request to CARROT's template_result create mapping"""
    return request_handler.create_map(
        "templates",
        template_id,
        "results",
        result_id,
        [("result_key", result_key), ("created_by", created_by)],
    )


def find_maps(
    template_id,
    result_id,
    result_key,
    created_before,
    created_after,
    created_by,
    sort,
    limit,
    offset,
):
    """Submits a request to CARROT's template_result find mapping"""
    # Create parameter list
    params = [
        ("result_id", result_id),
        ("result_key", result_key),
        ("created_before", created_before),
        ("created_after", created_after),
        ("created_by", created_by),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find_maps("templates", template_id, "results", params)


def find_map_by_ids(template_id, result_id):
    """Submits a request to CARROT's template_result find_by_id mapping"""
    return request_handler.find_map_by_ids(
        "templates", template_id, "results", result_id
    )


def delete_map_by_ids(template_id, result_id):
    """Submits a request to CARROT's template_result delete mapping"""
    return request_handler.delete_map_by_ids(
        "templates", template_id, "results", result_id
    )
