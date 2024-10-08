import logging

import click

from .. import command_util, dependency_util, email_util, file_util
from ..rest import report_maps, reports, runs

LOGGER = logging.getLogger(__name__)


@click.group(name="run")
def main():
    """Commands for searching, creating, and updating runs"""


@main.command(name="find_by_id")
@click.argument("id")
@click.option(
    "--zip_csv",
    "--instead_of_json_give_me_a_zipped_folder_with_csvs_in_it_please_and_thank_you",
    type=click.Path(),
    help="Instead of writing results to stdout as JSON, writes as a zip of CSV files to the specified file",
)
def find_by_id(id, zip_csv=None):
    """Retrieve a run by its ID"""
    print(runs.find_by_id(id, csv=zip_csv))


@main.command(name="delete")
@click.argument("run")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of run created by "
    "another user",
)
def delete(run, yes):
    """
    Delete the run specified by RUN (id or name), if the run has a failed status
    """
    # Process run to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        run, runs, "run_id", "run"
    )
    command_util.delete(id, yes, runs, "Run")


@main.command(name="create_report")
@click.argument("run")
@click.argument("report")
@click.option(
    "--created_by", default=None, type=str, help="Email of the creator of the mapping"
)
@click.option(
    "--delete_failed",
    is_flag=True,
    help="If set, and there is a failed record for this run with this report, will overwrite that "
    "record",
)
def create_report(run, report, created_by, delete_failed):
    """
    Start a job to generate a filled report using data from the run specified by RUN (id or name)
    with the report specified by REPORT (id or name)
    """
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    # Process run to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        run, runs, "run_id", "run"
    )
    # Do the same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    print(report_maps.create_map("runs", id, report_id, created_by, delete_failed))


@main.command(name="find_report_by_ids")
@click.argument("run")
@click.argument("report")
def find_report_by_ids(run, report):
    """
    Retrieve the report record for the run specified by RUN (id or name) and the report specified
    by REPORT (id or name)
    """
    # Process run to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        run, runs, "run_id", "run"
    )
    # Do the same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    print(report_maps.find_map_by_ids("runs", id, report_id))


@main.command(name="find_reports")
@click.argument("run")
@click.option(
    "--report",
    "--report_id",
    default=None,
    type=str,
    help="The id or name of the report",
)
@click.option(
    "--status",
    default=None,
    type=str,
    help="The status of the job generating the report",
)
@click.option(
    "--cromwell_job_id",
    default=None,
    type=str,
    help="The id for the cromwell job for generating the filled report",
)
@click.option(
    "--results",
    default=None,
    type=str,
    help="A json file containing the results of the report job",
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for report record's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for report record's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the report record, case sensitive",
)
@click.option(
    "--finished_before",
    default=None,
    type=str,
    help="Upper bound for report record's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--finished_after",
    default=None,
    type=str,
    help="Lower bound for report record's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--sort",
    default=None,
    type=str,
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
    run,
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
    Retrieve the report records for the run specified by RUN (id or name) for the specified params
    """
    # Process run to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        run, runs, "run_id", "run"
    )
    # Same for report
    if report:
        report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
            report, reports, "report_id", "report"
        )
    else:
        report_id = None
    print(
        report_maps.find_maps(
            "runs",
            id,
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
@click.argument("run")
@click.argument("report")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of report created by "
    "another user",
)
def delete_report_by_ids(run, report, yes):
    """
    Delete the report record for the run specified by RUN (id or name) to the report specified by
    REPORT (id or name)
    """
    # Process run to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        run, runs, "run_id", "run"
    )
    # Do the same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    command_util.delete_map(
        id, report_id, yes, report_maps, "run", "report", entity1_rest_name="runs"
    )
