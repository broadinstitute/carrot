import logging

import click

from ... import dependency_util
from ...rest import software_versions
from ...rest import software as software_rest
from .software_build import command as software_build

LOGGER = logging.getLogger(__name__)


@click.group(name="version")
def main():
    "Commands for querying software version records"


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a software version record by its ID"""
    print(software_versions.find_by_id(id))


@main.command(name="find")
@click.option(
    "--software_version_id",
    default="",
    help="The ID of the software version record",
)
@click.option(
    "--software",
    "--software_id",
    default="",
    help="The ID or name of the software to find version records of",
)
@click.option("--commit", default="", help="The commit hash for the version")
@click.option(
    "--created_before",
    default="",
    help="Upper bound for software version's created_at value, in the format "
    "YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for software version's created_at value, in the format "
    "YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--sort",
    default="",
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
    "descending.  Ex. asc(software_name),desc(created_at)",
)
@click.option(
    "--limit",
    default=20,
    show_default=True,
    help="The maximum number of software version records to return",
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
    software_version_id,
    software,
    commit,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve software version records filtered to match the specified parameters"""
    # Process software to get id if it's a name
    if software:
        software_id = dependency_util.get_id_from_id_or_name_and_handle_error(software, software_rest, "software_id", "software")
    else:
        software_id = ""
    print(
        software_versions.find(
            software_version_id,
            software_id,
            commit,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


main.add_command(software_build.main)
