import logging
import sys

import click

from .. import dependency_util
from ..rest import pipelines, subscriptions, templates, tests

LOGGER = logging.getLogger(__name__)


@click.group(name="subscription")
def main():
    """Commands for searching subscriptions"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a subscription by its ID"""
    print(subscriptions.find_by_id(id))


@main.command(name="find")
@click.option(
    "--subscription_id", default="", help="The subscription's ID"
)
@click.option(
    "--entity_type",
    default="",
    help="The type of the entity subscribed to (pipeline, template, or test)",
)
@click.option("--entity", "--entity_id", default="", help="The entity's ID or name")
@click.option(
    "--created_before",
    default="",
    help="Upper bound for subscription's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for subscription's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option("--email", default="", help="Email of the subscriber, case sensitive")
@click.option(
    "--sort",
    default="",
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
    "descending.  Ex. asc(entity_type),desc(entity_id)",
)
@click.option(
    "--limit",
    default=20,
    show_default=True,
    help="The maximum number of subscription records to return",
)
@click.option(
    "--offset",
    default=0,
    show_default=True,
    help="The offset to start at within the list of records to return.  Ex. Sorting by "
    "asc(created_at) with offset=1 would return records sorted by when they were created "
    "starting from the second record to be created",
)
def find(
    subscription_id,
    entity_type,
    entity,
    created_before,
    created_after,
    email,
    sort,
    limit,
    offset,
):
    """Retrieve subscriptions filtered to match the specified parameters"""
    # Process entity in case it's a name
    if entity_type.lower() == "pipeline":
        entity_id = dependency_util.get_id_from_id_or_name_and_handle_error(entity, pipelines, "pipeline_id", "pipeline")
    elif entity_type.lower() == "template":
        entity_id = dependency_util.get_id_from_id_or_name_and_handle_error(entity, templates, "template_id", "template")
    elif entity_type.lower() == "test":
        entity_id = dependency_util.get_id_from_id_or_name_and_handle_error(entity, tests, "test_id", "test")
    else:
        LOGGER.error("Invalid value for entity_type.  Must be pipeline, template, or test")
        sys.exit(1)

    print(
        subscriptions.find(
            subscription_id,
            entity_type,
            entity_id,
            created_before,
            created_after,
            email,
            sort,
            limit,
            offset,
        )
    )
