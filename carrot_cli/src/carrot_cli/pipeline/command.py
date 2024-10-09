import logging
import sys

import click

from .. import (
    command_util,
    dependency_util,
    email_util,
    file_util,
    software_version_query_util,
)
from ..config import manager as config
from ..rest import pipelines, report_maps, reports, runs

LOGGER = logging.getLogger(__name__)


@click.group(name="pipeline")
def main():
    """Commands for searching, creating, and updating pipelines"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a pipeline by its ID"""
    print(pipelines.find_by_id(id))


@main.command(name="find")
@click.option("--pipeline_id", default=None, type=str, help="The pipeline's ID")
@click.option(
    "--name", default=None, type=str, help="The name of the pipeline, case-sensitive"
)
@click.option(
    "--description",
    default=None,
    type=str,
    help="The description of the pipeline, case-sensitive",
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for pipeline's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for pipeline's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the pipeline, case sensitive",
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
    help="The maximum number of pipeline records to return",
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
    pipeline_id,
    name,
    description,
    created_by,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve pipelines filtered to match the specified parameters"""
    print(
        pipelines.find(
            pipeline_id,
            name,
            description,
            created_by,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="create")
@click.option("--name", help="The name of the pipeline", required=True)
@click.option(
    "--description", default=None, type=str, help="The description of the pipeline"
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the pipeline.  Defaults to email config variable",
)
def create(name, description, created_by):
    """Create pipeline with the specified parameters"""
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    print(pipelines.create(name, description, created_by))


@main.command(name="update")
@click.argument("pipeline")
@click.option("--name", default=None, type=str, help="The name of the pipeline")
@click.option(
    "--description", default=None, type=str, help="The description of the pipeline"
)
def update(pipeline, name, description):
    """Update pipeline specified by PIPELINE (id or name) with the specified parameters"""
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        pipeline, pipelines, "pipeline_id", "pipeline"
    )
    print(pipelines.update(id, name, description))


@main.command(name="delete")
@click.argument("pipeline")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of pipeline created by "
    "another user",
)
def delete(pipeline, yes):
    """Delete a pipeline by its id or name, if the pipeline has no templates associated with it."""
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        pipeline, pipelines, "pipeline_id", "pipeline"
    )
    command_util.delete(id, yes, pipelines, "Pipeline")


@main.command(name="find_runs")
@click.argument("pipeline")
@click.option(
    "--run_group_id",
    default=None,
    type=str,
    help="The id of the run group to which the run belongs",
)
@click.option("--name", default=None, type=str, help="The name of the run")
@click.option(
    "--status",
    default=None,
    type=str,
    help="The status of the run. Status include: aborted, building, created, failed, "
    "queued_in_cromwell, running, starting, submitted, succeeded, waiting_for_queue_space",
)
@click.option(
    "--test_input",
    default=None,
    type=str,
    help="A JSON file containing the inputs to the test WDL for the run",
)
@click.option(
    "--test_options",
    default=None,
    type=str,
    help="A JSON file containing the workflow options to the test WDL for the run",
)
@click.option(
    "--eval_input",
    default=None,
    type=str,
    help="A JSON file containing the inputs to the eval WDL for the run",
)
@click.option(
    "--eval_options",
    default=None,
    type=str,
    help="A JSON file containing the workflow options to the eval WDL for the run",
)
@click.option(
    "--test_cromwell_job_id",
    default=None,
    type=str,
    help="The unique ID assigned to the Cromwell job in which the test WDL ran",
)
@click.option(
    "--eval_cromwell_job_id",
    default=None,
    type=str,
    help="The unique ID assigned to the Cromwell job in which the eval WDL ran",
)
@click.option(
    "--software_name",
    default=None,
    type=str,
    help="The name of a software for which an image was built for the run.  Must be used in conjunction with either a "
    "list of commits/tags (--commits_and_tags), a count of commits on a branch (--commit_count and optionally "
    "--software_branch), or a date range for the commits (--commit_to and/or --commit_from and optionally "
    "--software_branch)",
)
@click.option(
    "--commit_or_tag",
    default=None,
    type=str,
    multiple=True,
    help="A commit or tag corresponding to the software specified using --software_name for which an image was built "
    "for the run.  Can be used multiple times to list multiple commits and/or tags.",
)
@click.option(
    "--commit_count",
    default=None,
    type=int,
    help="A count of the most recent commits (on --software_branch if specified) to the software specified using "
    "--software_name for which an image was built for the run.",
)
@click.option(
    "--commit_from",
    default=None,
    type=str,
    help="A lower bound (in the format YYYY-MM-DDThh:mm:ss.ssssss) of a range of commits (on --software_branch if "
    "specified) to the software specified using --software_name for which an image was built for the run.",
)
@click.option(
    "--commit_to",
    default=None,
    type=str,
    help="An upper bound (in the format YYYY-MM-DDThh:mm:ss.ssssss) of a range of commits (on --software_branch if "
    "specified) to the software specified using --software_name for which an image was built for the run.",
)
@click.option(
    "--software_branch",
    default=None,
    type=str,
    help="A branch on the software specified using --software_name from which to retrieve commits using "
    "--commit_count or --commit_from and/or --commit_to for which an image was built for the run.",
)
@click.option(
    "--tags_only",
    is_flag=True,
    help="If using --commit_count, specifies that the results should be the last n tags instead of the last n commits",
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for run's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for run's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by", default=None, type=str, help="Email of the creator of the run"
)
@click.option(
    "--finished_before",
    default=None,
    type=str,
    help="Upper bound for run's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--finished_after",
    default=None,
    type=str,
    help="Lower bound for run's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--sort",
    default=None,
    type=str,
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
    "descending.  Ex. asc(status),desc(created_at)",
)
@click.option(
    "--limit",
    default=20,
    show_default=True,
    help="The maximum number of run records to return",
)
@click.option(
    "--offset",
    default=0,
    show_default=True,
    help="The offset to start at within the list of records to return.  Ex. Sorting by "
    "asc(created_at) with offset=1 would return records sorted by when they were created "
    "starting from the second record to be created",
)
@click.option(
    "--zip_csv",
    "--instead_of_json_give_me_a_zipped_folder_with_csvs_in_it_please_and_thank_you",
    type=click.Path(),
    help="Instead of writing results to stdout as JSON, writes as a zip of CSV files to the specified file",
)
def find_runs(
    pipeline,
    run_group_id,
    name,
    status,
    test_input,
    test_options,
    eval_input,
    eval_options,
    test_cromwell_job_id,
    eval_cromwell_job_id,
    software_name,
    commit_or_tag,
    commit_count,
    commit_from,
    commit_to,
    software_branch,
    tags_only,
    created_before,
    created_after,
    created_by,
    finished_before,
    finished_after,
    sort,
    limit,
    offset,
    zip_csv,
):
    """
    Retrieve runs related to the pipeline specified by PIPELINE (id or name), filtered by the
    specified parameters
    """
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        pipeline, pipelines, "pipeline_id", "pipeline"
    )
    # Process software version query info into the proper format
    software_versions = software_version_query_util.get_software_version_query(
        software_name,
        commit_or_tag,
        commit_count,
        commit_from,
        commit_to,
        software_branch,
        tags_only,
    )
    print(
        runs.find(
            "pipelines",
            id,
            run_group_id,
            name,
            status,
            test_input,
            test_options,
            eval_input,
            eval_options,
            test_cromwell_job_id,
            eval_cromwell_job_id,
            software_versions,
            created_before,
            created_after,
            created_by,
            finished_before,
            finished_after,
            sort,
            limit,
            offset,
            csv=zip_csv,
        )
    )


@main.command(name="create_report_for_runs")
@click.argument("pipeline")
@click.argument("report")
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the report mapping (you)",
)
@click.option(
    "--run_group_id",
    default=None,
    type=str,
    help="The id of the run group to which the run belongs",
)
@click.option("--name", default=None, type=str, help="The name of the run")
@click.option(
    "--status",
    default=None,
    type=str,
    help="The status of the run. Status include: aborted, building, created, failed, "
    "queued_in_cromwell, running, starting, submitted, succeeded, waiting_for_queue_space",
)
@click.option(
    "--test_input",
    default=None,
    type=str,
    help="A JSON file containing the inputs to the test WDL for the run",
)
@click.option(
    "--test_options",
    default=None,
    type=str,
    help="A JSON file containing the workflow options to the test WDL for the run",
)
@click.option(
    "--eval_input",
    default=None,
    type=str,
    help="A JSON file containing the inputs to the eval WDL for the run",
)
@click.option(
    "--eval_options",
    default=None,
    type=str,
    help="A JSON file containing the workflow options to the eval WDL for the run",
)
@click.option(
    "--test_cromwell_job_id",
    default=None,
    type=str,
    help="The unique ID assigned to the Cromwell job in which the test WDL ran",
)
@click.option(
    "--eval_cromwell_job_id",
    default=None,
    type=str,
    help="The unique ID assigned to the Cromwell job in which the eval WDL ran",
)
@click.option(
    "--software_name",
    default=None,
    type=str,
    help="The name of a software for which an image was built for the run.  Must be used in conjunction with either a "
    "list of commits/tags (--commits_and_tags), a count of commits on a branch (--commit_count and optionally "
    "--software_branch), or a date range for the commits (--commit_to and/or --commit_from and optionally "
    "--software_branch)",
)
@click.option(
    "--commit_or_tag",
    default=None,
    type=str,
    multiple=True,
    help="A commit or tag corresponding to the software specified using --software_name for which an image was built "
    "for the run.  Can be used multiple times to list multiple commits and/or tags.",
)
@click.option(
    "--commit_count",
    default=None,
    type=int,
    help="A count of the most recent commits (on --software_branch if specified) to the software specified using "
    "--software_name for which an image was built for the run.",
)
@click.option(
    "--commit_from",
    default=None,
    type=str,
    help="A lower bound (in the format YYYY-MM-DDThh:mm:ss.ssssss) of a range of commits (on --software_branch if "
    "specified) to the software specified using --software_name for which an image was built for the run.",
)
@click.option(
    "--commit_to",
    default=None,
    type=str,
    help="An upper bound (in the format YYYY-MM-DDThh:mm:ss.ssssss) of a range of commits (on --software_branch if "
    "specified) to the software specified using --software_name for which an image was built for the run.",
)
@click.option(
    "--software_branch",
    default=None,
    type=str,
    help="A branch on the software specified using --software_name from which to retrieve commits using "
    "--commit_count or --commit_from and/or --commit_to for which an image was built for the run.",
)
@click.option(
    "--tags_only",
    is_flag=True,
    help="If using --commit_count, specifies that the results should be the last n tags instead of the last n commits",
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for run's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for run's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--run_created_by", default=None, type=str, help="Email of the creator of the run"
)
@click.option(
    "--finished_before",
    default=None,
    type=str,
    help="Upper bound for run's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--finished_after",
    default=None,
    type=str,
    help="Lower bound for run's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--sort",
    default=None,
    type=str,
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
    "descending.  Ex. asc(status),desc(created_at)",
)
@click.option(
    "--limit",
    default=20,
    show_default=True,
    help="The maximum number of run records to return",
)
@click.option(
    "--offset",
    default=0,
    show_default=True,
    help="The offset to start at within the list of records to return.  Ex. Sorting by "
    "asc(created_at) with offset=1 would return records sorted by when they were created "
    "starting from the second record to be created",
)
def create_report_for_runs(
    pipeline,
    report,
    created_by,
    run_group_id,
    name,
    status,
    test_input,
    test_options,
    eval_input,
    eval_options,
    test_cromwell_job_id,
    eval_cromwell_job_id,
    software_name,
    commit_or_tag,
    commit_count,
    commit_from,
    commit_to,
    software_branch,
    tags_only,
    created_before,
    created_after,
    run_created_by,
    finished_before,
    finished_after,
    sort,
    limit,
    offset,
):
    """
    Query for runs of the pipeline specified by PIPELINE (id or name), filtered by the specified parameters, then
    generate a filled report using the data from those runs with the report specified by REPORT (id or name)
    """
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        pipeline, pipelines, "pipeline_id", "pipeline"
    )
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)
    # Process software version query info into the proper format
    software_versions = software_version_query_util.get_software_version_query(
        software_name,
        commit_or_tag,
        commit_count,
        commit_from,
        commit_to,
        software_branch,
        tags_only,
    )
    print(
        report_maps.create_map_from_run_query(
            report_id,
            created_by,
            "pipelines",
            id,
            run_group_id,
            name,
            status,
            test_input,
            test_options,
            eval_input,
            eval_options,
            test_cromwell_job_id,
            eval_cromwell_job_id,
            software_versions,
            created_before,
            created_after,
            run_created_by,
            finished_before,
            finished_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="subscribe")
@click.argument("pipeline")
@click.option(
    "--email",
    default=None,
    type=str,
    help="The email address to receive notifications. If set, takes priority over email config "
    "variable",
)
def subscribe(pipeline, email):
    """Subscribe to receive notifications about the pipeline specified by PIPELINE (name or id)"""
    # If email is not set and there is an email config variable, fill with that
    if email is None:
        email_config_val = config.load_var_no_error("email")
        if email_config_val is not None:
            email = email_config_val
        # If the config variable is also not set, print a message to the user and exit
        else:
            LOGGER.error(
                "Subscribing requires that an email address is supplied either via the --email"
                "flag or by setting the email config variable"
            )
            sys.exit(1)
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        pipeline, pipelines, "pipeline_id", "pipeline"
    )
    print(pipelines.subscribe(id, email))


@main.command(name="unsubscribe")
@click.argument("pipeline")
@click.option(
    "--email",
    default=None,
    type=str,
    help="The email address to stop receiving notifications. If set, takes priority over email "
    "config variable",
)
def unsubscribe(pipeline, email):
    """Delete subscription to the pipeline with the specified by PIPELINE (id or name) and email"""
    # If email is not set and there is an email config variable, fill with that
    if email is None:
        email_config_val = config.load_var_no_error("email")
        if email_config_val is not None:
            email = email_config_val
        # If the config variable is also not set, print a message to the user and exit
        else:
            LOGGER.error(
                "Unsubscribing requires that an email address is supplied either via the --email"
                "flag or by setting the email config variable"
            )
            sys.exit(1)
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        pipeline, pipelines, "pipeline_id", "pipeline"
    )
    print(pipelines.unsubscribe(id, email))
