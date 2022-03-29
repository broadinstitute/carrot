import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(software_version_id):
    """Submits a request to CARROT's software_versions find_by_id mapping"""
    return request_handler.find_by_id("software_versions", software_version_id)


def find(
    software_version_id,
    software_id,
    commit,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Submits a request to CARROT's software_versions find mapping"""
    # Create parameter list
    params = [
        ("software_version_id", software_version_id),
        ("software_id", software_id),
        ("commit", commit),
        ("created_before", created_before),
        ("created_after", created_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("software_versions", params)
