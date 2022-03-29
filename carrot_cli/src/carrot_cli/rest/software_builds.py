import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(software_build_id):
    """Submits a request to CARROT's software_builds find_by_id mapping"""
    return request_handler.find_by_id("software_builds", software_build_id)


def find(
    software_build_id,
    software_version_id,
    build_job_id,
    status,
    image_url,
    created_before,
    created_after,
    finished_before,
    finished_after,
    sort,
    limit,
    offset,
):
    """Submits a request to CARROT's software_builds find mapping"""
    # Create parameter list
    params = [
        ("software_build_id", software_build_id),
        ("software_version_id", software_version_id),
        ("build_job_id", build_job_id),
        ("status", status),
        ("image_url", image_url),
        ("created_before", created_before),
        ("created_after", created_after),
        ("finished_before", finished_before),
        ("finished_after", finished_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("software_builds", params)
