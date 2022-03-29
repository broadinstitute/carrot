import logging

from .. import config
from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(pipeline_id):
    """Submits a request to CARROT's pipelines find_by_id mapping"""
    return request_handler.find_by_id("pipelines", pipeline_id)


def find(
    pipeline_id="",
    name="",
    description="",
    created_by="",
    created_before="",
    created_after="",
    sort="",
    limit="",
    offset="",
):
    """Submits a request to CARROT's pipelines find mapping"""
    # Create parameter list
    params = [
        ("pipeline_id", pipeline_id),
        ("name", name),
        ("description", description),
        ("created_by", created_by),
        ("created_before", created_before),
        ("created_after", created_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("pipelines", params)


def create(name, description, created_by):
    """Submits a request to CARROT's pipelines create mapping"""
    # Create parameter list
    params = [("name", name), ("description", description), ("created_by", created_by)]
    return request_handler.create("pipelines", params)


def update(pipeline_id, name, description):
    """Submits a request to CARROT's pipelines update mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("description", description),
    ]
    return request_handler.update("pipelines", pipeline_id, params)


def delete(pipeline_id):
    """Submits a request to CARROT's pipelines delete mapping"""
    return request_handler.delete("pipelines", pipeline_id)


def subscribe(pipeline_id, email):
    """Submits a request to CARROT's pipelines subscribe mapping"""
    return request_handler.subscribe("pipelines", pipeline_id, email)


def unsubscribe(pipeline_id, email):
    """Submits a request to CARROT's pipelines unsubscribe mapping"""
    return request_handler.unsubscribe("pipelines", pipeline_id, email)
