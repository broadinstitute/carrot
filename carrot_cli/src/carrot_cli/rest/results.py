import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(result_id):
    """Submits a request to CARROT's results find_by_id mapping"""
    return request_handler.find_by_id("results", result_id)


def find(
    result_id="",
    name="",
    description="",
    result_type="",
    created_by="",
    created_before="",
    created_after="",
    sort="",
    limit="",
    offset="",
):
    """Submits a request to CARROT's results find mapping"""
    # Create parameter list
    params = [
        ("result_id", result_id),
        ("name", name),
        ("description", description),
        ("result_type", result_type),
        ("created_by", created_by),
        ("created_before", created_before),
        ("created_after", created_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("results", params)


def create(name, description, result_type, created_by):
    """Submits a request to CARROT's results create mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("description", description),
        ("result_type", result_type),
        ("created_by", created_by),
    ]
    return request_handler.create("results", params)


def update(result_id, name, description):
    """Submits a request to CARROT's results update mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("description", description),
    ]
    return request_handler.update("results", result_id, params)


def delete(result_id):
    """Submits a request to CARROT's results delete mapping"""
    return request_handler.delete("results", result_id)
