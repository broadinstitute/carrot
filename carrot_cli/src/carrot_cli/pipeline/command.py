import json
import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from .. import file_util
from ..config import manager as config
from ..rest import pipelines, runs

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
@click.option("--pipeline_id", default="", help="The pipeline's ID")
@click.option("--name", default="", help="The name of the pipeline, case-sensitive")
@click.option(
    "--description", default="", help="The description of the pipeline, case-sensitive"
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for pipeline's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for pipeline's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the pipeline, case sensitive",
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
@click.option("--description", default="", help="The description of the pipeline")
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the pipeline.  Defaults to email config variable",
)
def create(name, description, created_by):
    """Create pipeline with the specified parameters"""
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
    print(pipelines.create(name, description, created_by))


@main.command(name="update")
@click.argument("pipeline")
@click.option("--name", default="", help="The name of the pipeline")
@click.option("--description", default="", help="The description of the pipeline")
def update(pipeline, name, description):
    """Update pipeline specified by PIPELINE (id or name) with the specified parameters"""
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")
    command_util.delete(id, yes, pipelines, "Pipeline")


@main.command(name="find_runs")
@click.argument("pipeline")
@click.option("--name", default="", help="The name of the run")
@click.option(
    "--status",
    default="",
    help="The status of the run. Status include: aborted, building, created, failed, "
    "queued_in_cromwell, running, starting, submitted, succeeded, waiting_for_queue_space",
)
@click.option(
    "--test_input",
    default="",
    help="A JSON file containing the inputs to the test WDL for the run",
)
@click.option(
    "--test_options",
    default="",
    help="A JSON file containing the workflow options to the test WDL for the run",
)
@click.option(
    "--eval_input",
    default="",
    help="A JSON file containing the inputs to the eval WDL for the run",
)
@click.option(
    "--eval_options",
    default="",
    help="A JSON file containing the workflow options to the eval WDL for the run",
)
@click.option(
    "--test_cromwell_job_id",
    default="",
    help="The unique ID assigned to the Cromwell job in which the test WDL ran",
)
@click.option(
    "--eval_cromwell_job_id",
    default="",
    help="The unique ID assigned to the Cromwell job in which the eval WDL ran",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for run's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for run's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option("--created_by", default="", help="Email of the creator of the run")
@click.option(
    "--finished_before",
    default="",
    help="Upper bound for run's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--finished_after",
    default="",
    help="Lower bound for run's finished_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--sort",
    default="",
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
def find_runs(
    pipeline,
    name,
    status,
    test_input,
    test_options,
    eval_input,
    eval_options,
    test_cromwell_job_id,
    eval_cromwell_job_id,
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
    Retrieve runs related to the pipeline specified by PIPELINE (id or name), filtered by the
    specified parameters
    """
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)
    # Process pipeline to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")
    print(
        runs.find(
            "pipelines",
            id,
            name,
            status,
            test_input,
            test_options,
            eval_input,
            eval_options,
            test_cromwell_job_id,
            eval_cromwell_job_id,
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


@main.command(name="subscribe")
@click.argument("pipeline")
@click.option(
    "--email",
    default="",
    help="The email address to receive notifications. If set, takes priority over email config "
    "variable",
)
def subscribe(pipeline, email):
    """Subscribe to receive notifications about the pipeline specified by PIPELINE (name or id)"""
    # If email is not set and there is an email config variable, fill with that
    if email == "":
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")
    print(pipelines.subscribe(id, email))


@main.command(name="unsubscribe")
@click.argument("pipeline")
@click.option(
    "--email",
    default="",
    help="The email address to stop receiving notifications. If set, takes priority over email "
    "config variable",
)
def unsubscribe(pipeline, email):
    """Delete subscription to the pipeline with the specified by PIPELINE (id or name) and email"""
    # If email is not set and there is an email config variable, fill with that
    if email == "":
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")
    print(pipelines.unsubscribe(id, email))
