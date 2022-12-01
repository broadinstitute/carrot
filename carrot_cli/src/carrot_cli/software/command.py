import logging
import sys

import click

from .. import dependency_util
from .. import email_util
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
@click.option("--software_id", default=None, type=str, help="The software's ID")
@click.option("--name", default=None, type=str, help="The name of the software, case-sensitive")
@click.option(
    "--description", default=None, type=str, help="The description of the software, case-sensitive"
)
@click.option(
    "--repository_url",
    default=None,
    type=str,
    help="The url of the repository where the software code is hosted",
)
@click.option(
    "--machine_type",
    default=None,
    help="Optional machine type for Google Cloud Build to use for building this software.",
    type=click.Choice(["standard", "n1-highcpu-8", "n1-highcpu-32", "e2-highcpu-8", "e2-highcpu-32", ''], case_sensitive=False)
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for software's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for software's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the software, case sensitive",
)
@click.option(
    "--sort",
    default=None,
    type=str,
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
    machine_type,
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
            machine_type,
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
@click.option("--description", default=None, type=str, help="The description of the software")
@click.option(
    "--repository_url",
    default=None,
    type=str,
    help="The url to use for cloning the repository.",
    required=True,
)
@click.option(
    "--machine_type",
    default="",
    help="Optional machine type for Google Cloud Build to use for building this software.",
    type=click.Choice(["standard", "n1-highcpu-8", "n1-highcpu-32", "e2-highcpu-8", "e2-highcpu-32", ""], case_sensitive=False)
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the software.  Defaults to email config variable",
)
def create(name, description, repository_url, machine_type, created_by):
    """Create software definition with the specified parameters"""
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    print(software_rest.create(name, description, repository_url, machine_type, created_by))


@main.command(name="update")
@click.argument("software")
@click.option("--name", default=None, type=str, help="The name of the software")
@click.option("--description", default=None, type=str, help="The description of the software")
@click.option(
    "--machine_type",
    default="",
    help="Optional machine type for Google Cloud Build to use for building this software.",
    type=click.Choice(["standard", "n1-highcpu-8", "n1-highcpu-32", "e2-highcpu-8", "e2-highcpu-32", ""], case_sensitive=False)
)
def update(software, name, description, machine_type):
    """Update software definition for SOFTWARE (id or name) with the specified parameters"""
    # Process software to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(software, software_rest, "software_id", "software")
    if machine_type == "":
        machine_type = None
    print(software_rest.update(id, name, description, machine_type))


main.add_command(software_version.main)
