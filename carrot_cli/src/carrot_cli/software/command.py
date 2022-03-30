import logging
import sys

import click

from .. import dependency_util
from ..config import manager as config
from ..rest import software as software_rest
from .software_version import command as software_version

LOGGER = logging.getLogger(__name__)


@click.group(name="software")
def main():
    """Commands for searching, creating, and updating software definitions"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a software definition by its ID"""
    print(software_rest.find_by_id(id))


@main.command(name="find")
@click.option("--software_id", default="", help="The software's ID")
@click.option("--name", default="", help="The name of the software, case-sensitive")
@click.option(
    "--description", default="", help="The description of the software, case-sensitive"
)
@click.option(
    "--repository_url",
    default="",
    help="The url of the repository where the software code is hosted",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for software's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for software's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the software, case sensitive",
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
    help="The maximum number of software records to return",
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
    software_id,
    name,
    description,
    repository_url,
    created_by,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve software definitions filtered to match the specified parameters"""
    print(
        software_rest.find(
            software_id,
            name,
            description,
            repository_url,
            created_by,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="create")
@click.option("--name", help="The name of the software", required=True)
@click.option("--description", default="", help="The description of the software")
@click.option(
    "--repository_url",
    default="",
    help="The url to use for cloning the repository.",
    required=True,
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the software.  Defaults to email config variable",
)
def create(name, description, repository_url, created_by):
    """Create software definition with the specified parameters"""
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
    print(software_rest.create(name, description, repository_url, created_by))


@main.command(name="update")
@click.argument("software")
@click.option("--name", default="", help="The name of the software")
@click.option("--description", default="", help="The description of the software")
def update(software, name, description):
    """Update software definition for SOFTWARE (id or name) with the specified parameters"""
    # Process software to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(software, software_rest, "software_id", "software")
    print(software_rest.update(id, name, description))


main.add_command(software_version.main)
