import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(subscription_id):
    """Submits a request to CARROT's subscriptions find_by_id mapping"""
    return request_handler.find_by_id("subscriptions", subscription_id)


def find(
    subscription_id,
    entity_type,
    entity_id,
    created_before,
    created_after,
    email,
    sort,
    limit,
    offset,
):
    """Submits a request to CARROT's subscriptions find mapping"""
    # Create parameter list
    params = [
        ("subscription_id", subscription_id),
        ("entity_type", entity_type),
        ("entity_id", entity_id),
        ("created_before", created_before),
        ("created_after", created_after),
        ("email", email),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("subscriptions", params)
