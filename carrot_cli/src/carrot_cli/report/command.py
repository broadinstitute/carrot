import json
import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from .. import file_util

# Naming this differently here than in other files because reports have a config attribute
from ..config import manager as config_manager
from ..rest import reports

LOGGER = logging.getLogger(__name__)


@click.group(name="report")
def main():
    """Commands for searching, creating, and updating reports"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a report by its ID"""
    print(reports.find_by_id(id))


@main.command(name="find")
@click.option("--report_id", default="", help="The report's ID")
@click.option("--name", default="", help="The name of the report, case-sensitive")
@click.option(
    "--description", default="", help="The description of the report, case-sensitive"
)
@click.option(
    "--notebook",
    default="",
    help="The ipynb file containing the notebook for the report.",
)
@click.option(
    "--config",
    default="",
    help="A json file containing values for runtime attributes for the Cromwell job that runs "
    "the report.",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for report's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for report's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the report, case sensitive",
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
    help="The maximum number of report records to return",
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
    report_id,
    name,
    description,
    notebook,
    config,
    created_by,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve reports filtered to match the specified parameters"""
    print(
        reports.find(
            report_id,
            name,
            description,
            file_util.read_file_to_json(notebook),
            file_util.read_file_to_json(config),
            created_by,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="create")
@click.option("--name", help="The name of the report", required=True)
@click.option("--description", default="", help="The description of the report")
@click.option(
    "--notebook",
    default="",
    required=True,
    help="The ipynb file containing the notebook which will serve as a template for this report.",
)
@click.option(
    "--config",
    default="",
    help="A json file containing values for runtime attributes for the Cromwell job that will "
    "generate the report.  The allowed attributes are: cpu, memory, disks, docker, maxRetries, "
    "continueOnReturnCode, failOnStderr, preemptible, and bootDiskSizeGb.",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the report.  Defaults to email config variable",
)
def create(name, description, notebook, config, created_by):
    """Create report with the specified parameters"""
    # If created_by is not set and there is an email config variable, fill with that
    if created_by == "":
        email_config_val = config_manager.load_var_no_error("email")
        if email_config_val is not None:
            created_by = email_config_val
        else:
            LOGGER.error(
                "No email config variable set.  If a value is not specified for --created by, "
                "there must be a value set for email."
            )
            sys.exit(1)
    print(
        reports.create(
            name,
            description,
            file_util.read_file_to_json(notebook),
            file_util.read_file_to_json(config),
            created_by,
        )
    )


@main.command(name="update")
@click.argument("report")
@click.option("--name", default="", help="The name of the report")
@click.option("--description", default="", help="The description of the report")
@click.option(
    "--notebook",
    default="",
    help="The ipynb file containing the notebook which will serve as a template for this report.",
)
@click.option(
    "--config",
    default="",
    help="A json file containing values for runtime attributes for the Cromwell job that will "
    "generate the report.  The allowed attributes are: cpu, memory, disks, docker, maxRetries, "
    "continueOnReturnCode, failOnStderr, preemptible, and bootDiskSizeGb.",
)
def update(report, name, description, notebook, config):
    """Update report specified by REPORT (id or name) with the specified parameters"""
    # Process report to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    print(
        reports.update(
            id,
            name,
            description,
            file_util.read_file_to_json(notebook),
            file_util.read_file_to_json(config),
        )
    )


@main.command(name="delete")
@click.argument("report")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of report created by "
    "another user",
)
def delete(report, yes):
    """
    Delete a report specified by REPORT (id or name), if the report has no templates, sections, or
    runs associated with it.
    """
    # Process report to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    command_util.delete(id, yes, reports, "Report")
