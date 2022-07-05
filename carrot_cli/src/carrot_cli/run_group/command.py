import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from .. import file_util
from ..config import manager as config
from ..rest import report_maps, reports, run_groups

LOGGER = logging.getLogger(__name__)

@click.group(name="run_group")
def main():
    """Commands for searching, creating, and updating runs"""

@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a run group by its ID"""
    print(run_groups.find_by_id(id))

@main.command(name="find")
@click.option("--run_group_id", default="", help="The run group's ID")
@click.option("--owner",
    default="",
    help="The owner of the github repo from which this group was created"
)
@click.option(
    "--repo",
    default="",
    help="The github repository from which this group was created"
)
@click.option(
    "--issue_number",
    default="",
    help="The issue number for the github pull request on which the comment which created this "
         "group was posted"
)
@click.option(
    "--author",
    default="",
    help="The github username of the author of the comment that created this group"
)
@click.option(
    "--base_commit",
    default="",
    help="The commit hash for the base branch of the github pull request on which the comment "
         "that created this group was posted"
)
@click.option(
    "--head_commit",
    default="",
    help="The commit hash for the head branch of the github pull request on which the comment "
         "that created this group was posted"
)
@click.option(
    "--test_input_key",
    default="",
    help="The input key (if provided) to the test WDL that accepts the custom docker image built "
         "from the repo from which this group was created"
)
@click.option(
    "--eval_input_key",
    default="",
    help="The input key (if provided) to the eval WDL that accepts the custom docker image built "
         "from the repo from which this group was created"
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
        run_group_id,
        owner,
        repo,
        issue_number,
        author,
        base_commit,
        head_commit,
        test_input_key,
        eval_input_key,
        created_before,
        created_after,
        sort,
        limit,
        offset,
):
    """Retrieve results filtered to match the specified parameters"""
    print(
        run_groups.find(
            run_group_id,
            owner,
            repo,
            issue_number,
            author,
            base_commit,
            head_commit,
            test_input_key,
            eval_input_key,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )

@main.command(name="delete")
@click.argument("run_group_id")
def delete(run_group_id):
    """
    Delete the run group specified by RUN_GROUP_ID
    """
    print(run_groups.delete(run_group_id))

@main.command(name="create_report")
@click.argument("run_group_id")
@click.argument("report")
@click.option("--created_by", default="", help="Email of the creator of the mapping")
@click.option(
    "--delete_failed",
    is_flag=True,
    help="If set, and there is a failed record for this run group with this report, will overwrite that "
         "record",
)
def create_report(run_group_id, report, created_by, delete_failed):
    """
    Start a job to generate a filled report using data from the run group specified by RUN_GROUP_ID
    with the report specified by REPORT (id or name)
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
    # Do the same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    print(report_maps.create_map("run-groups", run_group_id, report_id, created_by, delete_failed))

@main.command(name="find_report_by_ids")
@click.argument("run_group_id")
@click.argument("report")
def find_report_by_ids(run_group_id, report):
    """
    Retrieve the report record for the run specified by RUN_GROUP_ID and the report specified
    by REPORT (id or name)
    """
    # Do the same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    print(report_maps.find_map_by_ids("run-groups", run_group_id, report_id))

@main.command(name="find_reports")
@click.argument("run_group_id")
@click.option("--report", "--report_id", default="", help="The id or name of the report")
@click.option(
    "--status", default="", help="The status of the job generating the report"
)
@click.option(
    "--cromwell_job_id",
    default="",
    help="The id for the cromwell job for generating the filled report",
)
@click.option(
    "--results",
    default="",
    help="A json file containing the results of the report job",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for report record's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for report record's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the report record, case sensitive",
)
@click.option(
    "--finished_before",
    default="",
    help="Upper bound for report record's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--finished_after",
    default="",
    help="Lower bound for report record's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--sort",
    default="",
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
         "descending.  Ex. asc(input_map),desc(report_id)",
)
@click.option(
    "--limit",
    default=20,
    show_default=True,
    help="The maximum number of map records to return",
)
@click.option(
    "--offset",
    default=0,
    show_default=True,
    help="The offset to start at within the list of records to return.  Ex. Sorting by "
         "asc(created_at) with offset=1 would return records sorted by when they were created "
         "starting from the second record to be created",
)
def find_reports(
        run_group_id,
        report,
        status,
        cromwell_job_id,
        results,
        created_before,
        created_after,
        created_by,
        finished_before,
        finished_after,
        sort,
        limit,
        offset,
):
    """
    Retrieve the report records for the run group specified by RUN_GROUP_ID for the specified params
    """
    # Process report to get id if it's a name
    if report:
        report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    else:
        report_id = ""
    print(
        report_maps.find_maps(
            "run-groups",
            run_group_id,
            report_id,
            status,
            cromwell_job_id,
            file_util.read_file_to_json(results),
            created_before,
            created_after,
            created_by,
            finished_before,
            finished_after,
            sort,
            limit,
            offset,
        )
    )

@main.command(name="delete_report_by_ids")
@click.argument("run_group_id")
@click.argument("report")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of report created by "
         "another user",
)
def delete_report_by_ids(run_group_id, report, yes):
    """
    Delete the mapping record for the run group specified by RUN_GROUP_ID to the report specified by
    REPORT (id or name)
    """
    # Process report to get id if it's a name
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    command_util.delete_map(run_group_id, report_id, yes, report_maps, "run-group", "report", entity1_rest_name="run-groups")