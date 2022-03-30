import json
import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from ..config import manager as config
from ..rest import results, template_results, templates

LOGGER = logging.getLogger(__name__)


@click.group(name="result")
def main():
    """Commands for searching, creating, and updating result definitions"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a result definition by its ID"""
    print(results.find_by_id(id))


@main.command(name="find")
@click.option("--result_id", default="", help="The result's ID")
@click.option("--name", default="", help="The name of the result, case-sensitive")
@click.option(
    "--description", default="", help="The description of the result, case-sensitive"
)
@click.option(
    "--result_type", default="", help="The type of the result: numeric, file, or text"
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for result's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for result's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the result, case-sensitive",
)
@click.option(
    "--sort",
    default="",
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
    "descending.  Ex. asc(name),desc(created_at)",
)
@click.option(
    "--limit",
    default=20,
    show_default=True,
    help="The maximum number of result records to return",
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
    result_id,
    name,
    description,
    result_type,
    created_by,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve results filtered to match the specified parameters"""
    print(
        results.find(
            result_id,
            name,
            description,
            result_type,
            created_by,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="create")
@click.option("--name", help="The name of the result", required=True)
@click.option("--description", default="", help="The description of the result")
@click.option(
    "--result_type",
    help="The type of the result: numeric, file, or text",
    required=True,
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the result.  Defaults to email config variable",
)
def create(name, description, result_type, created_by):
    """Create result with the specified parameters"""
    # If created_by is not set and there is an email config variable, fill with that
    if created_by == "":
        email_config_val = config.load_var_no_error("email")
        if email_config_val is not None:
            created_by = email_config_val
        else:
            LOGGER.error(
                "No email config variable set.  If a value is not specified for --created by, "
                "there must be a value set for email."
            )
            sys.exit(1)
    print(results.create(name, description, result_type, created_by))


@main.command(name="update")
@click.argument("result")
@click.option("--name", default="", help="The name of the result")
@click.option("--description", default="", help="The description of the result")
def update(result, name, description):
    """Update result for RESULT (id or name) with the specified parameters"""
    # Process result to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
    print(results.update(id, name, description))


@main.command(name="delete")
@click.argument("result")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of result created by "
    "another user",
)
def delete(result, yes):
    """
    Delete a result definition specified by RESULT (id or name), if the result is not mapped to
    any templates
    """
    # Process result to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
    command_util.delete(id, yes, results, "Result")


@main.command(name="map_to_template")
@click.argument("result")
@click.argument("template")
@click.argument("result_key")
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the mapping.  Defaults to email config variable",
)
def map_to_template(result, template, result_key, created_by):
    """
    Map the result specified by RESULT (id or name) to the template specified by TEMPLATE (id or
    name) for RESULT_KEY in the output generated by that template
    """
    # If created_by is not set and there is an email config variable, fill with that
    if created_by == "":
        email_config_val = config.load_var_no_error("email")
        if email_config_val is not None:
            created_by = email_config_val
        else:
            LOGGER.error(
                "No email config variable set.  If a value is not specified for --created by, "
                "there must be a value set for email."
            )
            sys.exit(1)
    # Process result to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
    # Same for template
    template_id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    print(template_results.create_map(template_id, id, result_key, created_by))
