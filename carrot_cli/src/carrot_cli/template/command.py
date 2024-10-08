import json
import logging
import sys

import click

from .. import (command_util, dependency_util, email_util, file_util,
                software_version_query_util)
from ..config import manager as config
from ..rest import (pipelines, report_maps, reports, results, runs,
                    template_reports, template_results, templates)

LOGGER = logging.getLogger(__name__)


@click.group(name="template")
def main():
    """Commands for searching, creating, and updating templates"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a template by its ID"""
    print(templates.find_by_id(id))


@main.command(name="find")
@click.option("--template_id", default=None, type=str, help="The template's ID")
@click.option(
    "--pipeline",
    "--pipeline_id",
    "--pipeline_name",
    default=None,
    type=str,
    help="The ID or name of the pipeline that is the template's parent",
)
@click.option(
    "--name", default=None, type=str, help="The name of the template, case-sensitive"
)
@click.option(
    "--description",
    default=None,
    type=str,
    help="The description of the template, case-sensitive",
)
@click.option(
    "--test_wdl",
    default=None,
    type=str,
    help="The location where the test WDL for the template is hosted, either in the form of a "
    "http/https/gs uri",
)
@click.option(
    "--eval_wdl",
    default=None,
    type=str,
    help="The location where the eval WDL for the template is hosted, either in the form of a "
    "http/https/gs uri",
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for template's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for template's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the template, case sensitive",
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
    help="The maximum number of template records to return",
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
    template_id,
    pipeline,
    name,
    description,
    test_wdl,
    eval_wdl,
    created_by,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve templates filtered to match the specified parameters"""
    # Process pipeline in case it's a name
    if pipeline:
        pipeline_id = dependency_util.get_id_from_id_or_name_and_handle_error(
            pipeline, pipelines, "pipeline_id", "pipeline"
        )
    else:
        pipeline_id = None
    print(
        templates.find(
            template_id,
            pipeline_id,
            name,
            description,
            test_wdl,
            eval_wdl,
            created_by,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="create")
@click.option(
    "--pipeline",
    "--pipeline_id",
    help="The ID or name of the pipeline that will be this template's parent",
    default=None,
    type=str,
)
@click.option("--name", help="The name of the template", default=None, type=str)
@click.option(
    "--description", default=None, type=str, help="The description of the template"
)
@click.option(
    "--test_wdl",
    help="The location where the test WDL for this template is hosted, or its local file path. The"
    "test WDL is the WDL which defines the thing the be tested",
    default=None,
    type=str,
)
@click.option(
    "--test_wdl_dependencies",
    help="The location where the test WDL dependencies zip for this template is hosted, or its"
    "local file path. The zip should be formatted the same as it would be for cromwell",
    default=None,
    type=str,
)
@click.option(
    "--eval_wdl",
    help="The location where the eval WDL for ths template is hosted.  The eval WDL is the WDL "
    "which takes the outputs from the test WDL and evaluates them",
    default=None,
    type=str,
)
@click.option(
    "--eval_wdl_dependencies",
    help="The location where the eval WDL dependencies zip for this template is hosted, or its"
    "local file path. The zip should be formatted the same as it would be for cromwell",
    default=None,
    type=str,
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the template.  Defaults to email config variable",
)
@click.option(
    "--copy",
    default=None,
    type=str,
    help="Name or ID of a template you'd like to copy.  If this is specified, a new template will be created with all "
    "the attributes of the copied template, except any attributes that you have specified.  If a name is not"
    " specified, the new template will be named in the format '{old_template_name}_copy'.",
)
def create(
    name,
    pipeline,
    description,
    test_wdl,
    test_wdl_dependencies,
    eval_wdl,
    eval_wdl_dependencies,
    created_by,
    copy,
):
    """Create template with the specified parameters"""
    # If copy is specified, get if it's a name
    if copy is not None:
        copy = dependency_util.get_id_from_id_or_name_and_handle_error(
            copy, templates, "template_id", "copy"
        )
    # If copy is not specified, make sure name, pipeline, test_wdl, and eval_Wdl have been specified
    if copy is None and (
        name is None or pipeline is None or test_wdl is None or eval_wdl is None
    ):
        LOGGER.error(
            "If a value is not specified for '--copy', then '--name', '--pipeline', '--test_wdl', and '--eval_wdl are"
            " required."
        )
        sys.exit(1)
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    # Process pipeline to get id if it's a name
    if pipeline is not None:
        pipeline_id = dependency_util.get_id_from_id_or_name_and_handle_error(
            pipeline, pipelines, "pipeline_id", "pipeline"
        )
    else:
        pipeline_id = None

    print(
        templates.create(
            name,
            pipeline_id,
            description,
            test_wdl,
            test_wdl_dependencies,
            eval_wdl,
            eval_wdl_dependencies,
            created_by,
            copy,
        )
    )


@main.command(name="update")
@click.argument("template")
@click.option("--name", default=None, type=str, help="The new name of the template")
@click.option(
    "--description", default=None, type=str, help="The new description of the template"
)
@click.option(
    "--test_wdl",
    default=None,
    type=str,
    help="The location where the new test WDL for the template is hosted or a local file path.  "
    "Updating this parameter is allowed only if the specified template has no non-failed "
    "(i.e. successful or currently running) runs associated with it",
)
@click.option(
    "--test_wdl_dependencies",
    default=None,
    type=str,
    help="The location where the new test WDL dependencies zip for the template is hosted or a "
    "local file path.  Updating this parameter is allowed only if the specified template has no "
    "non-failed (i.e. successful or currently running) runs associated with it",
)
@click.option(
    "--eval_wdl",
    default=None,
    type=str,
    help="The location where the new eval WDL for the template is hosted or a local file path.  "
    "Updating this parameter is allowed only if the specified template has no non-failed (i.e. "
    "successful or currently running) runs associated with it",
)
@click.option(
    "--eval_wdl_dependencies",
    default=None,
    type=str,
    help="The location where the new eval WDL dependencies zip for the template is hosted or a "
    "local file path.  Updating this parameter is allowed only if the specified template has no "
    "non-failed (i.e. successful or currently running) runs associated with it",
)
def update(
    template,
    name,
    description,
    test_wdl,
    test_wdl_dependencies,
    eval_wdl,
    eval_wdl_dependencies,
):
    """Update template with TEMPLATE (id or name) with the specified parameters"""
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )

    print(
        templates.update(
            id,
            name,
            description,
            test_wdl,
            test_wdl_dependencies,
            eval_wdl,
            eval_wdl_dependencies,
        )
    )


@main.command(name="delete")
@click.argument("template")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of template created by "
    "another user",
)
def delete(template, yes):
    """Delete a template by its ID or name, if it has no tests associated with it"""
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )

    command_util.delete(id, yes, templates, "Template")


@main.command(name="find_runs")
@click.argument("template")
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
    template,
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
    Retrieve runs related to the template specified by TEMPLATE (id or name), filtered by the
    specified parameters
    """
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)

    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
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
            "templates",
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
@click.argument("template")
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
    template,
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
    Query for runs of the template specified by TEMPLATE (id or name), filtered by the specified parameters, then
    generate a filled report using the data from those runs with the report specified by REPORT (id or name)
    """
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
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
            "templates",
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
@click.argument("template")
@click.option(
    "--email",
    default=None,
    type=str,
    help="The email address to receive notifications. If set, takes priority over email config "
    "variable",
)
def subscribe(template, email):
    """Subscribe to receive notifications about the template specified by TEMPLATE (id or name)"""
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
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )

    print(templates.subscribe(id, email))


@main.command(name="unsubscribe")
@click.argument("template")
@click.option(
    "--email",
    default=None,
    type=str,
    help="The email address to stop receiving notifications. If set, takes priority over email "
    "config variable",
)
def unsubscribe(template, email):
    """Delete subscription to the template specified by TEMPLATE (id or name) and email"""
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
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )

    print(templates.unsubscribe(id, email))


@main.command(name="map_to_result")
@click.argument("template")
@click.argument("result")
@click.argument("result_key")
@click.option(
    "--created_by", default=None, type=str, help="Email of the creator of the mapping"
)
def map_to_result(template, result, result_key, created_by):
    """
    Map the template specified by TEMPLATE (id or name) to the result specified by RESULT (id or
    name) for RESULT_KEY in in the output generated by that template
    """
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for result
    result_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        result, results, "result_id", "result"
    )
    print(template_results.create_map(id, result_id, result_key, created_by))


@main.command(name="find_result_map_by_id")
@click.argument("template")
@click.argument("result")
def find_result_map_by_id(template, result):
    """
    Retrieve the mapping record from the template specified by TEMPLATE (id or name) to the result
    specified by RESULT (id or name)
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for result
    result_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        result, results, "result_id", "result"
    )
    print(template_results.find_map_by_ids(id, result_id))


@main.command(name="find_result_maps")
@click.argument("template")
@click.option(
    "--result",
    "--result_id",
    default=None,
    type=str,
    help="The id or name of the result",
)
@click.option(
    "--result_key",
    default=None,
    type=str,
    help="The key used to name the result within the output of the template",
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the map, case sensitive",
)
@click.option(
    "--sort",
    default=None,
    type=str,
    help="A comma-separated list of sort keys, enclosed in asc() for ascending or desc() for "
    "descending.  Ex. asc(result_key),desc(result_id)",
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
def find_result_maps(
    template,
    result,
    result_key,
    created_before,
    created_after,
    created_by,
    sort,
    limit,
    offset,
):
    """
    Retrieve the mapping record from the template specified by ID to the result specified by
    RESULT_ID
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for result
    if result:
        result_id = dependency_util.get_id_from_id_or_name_and_handle_error(
            result, results, "result_id", "result"
        )
    else:
        result_id = None
    print(
        template_results.find_maps(
            id,
            result_id,
            result_key,
            created_before,
            created_after,
            created_by,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="delete_result_map_by_id")
@click.argument("template")
@click.argument("result")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of mapping created by "
    "another user",
)
def delete_result_map_by_id(template, result, yes):
    """
    Delete the mapping record from the template specified by TEMPLATE (id or name) to the result
    specified by RESULT (id or name), if the specified template has no non-failed (i.e. successful
    or currently running) runs associated with it
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for result
    result_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        result, results, "result_id", "result"
    )
    command_util.delete_map(id, result_id, yes, template_results, "template", "result")


@main.command(name="map_to_report")
@click.argument("template")
@click.argument("report")
@click.argument(
    "report_trigger",
    default="single",
    type=click.Choice(["single", "pr"], case_sensitive=False),
)
@click.option(
    "--created_by", default=None, type=str, help="Email of the creator of the mapping"
)
def map_to_report(template, report, report_trigger, created_by):
    """
    Map the template specified by TEMPLATE (id or name) to the report specified by REPORT (id or
    name)
    """
    # If created_by is not set and there is an email config variable, fill with that
    created_by = email_util.check_created_by(created_by)
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    print(template_reports.create_map(id, report_id, report_trigger, created_by))


@main.command(name="find_report_map_by_id")
@click.argument("template")
@click.argument("report")
@click.argument(
    "report_trigger", type=click.Choice(["single", "pr"], case_sensitive=False)
)
def find_report_map_by_id(template, report, report_trigger):
    """
    Retrieve the mapping record from the template specified by TEMPLATE (id or name) to the report
    specified by REPORT (id or name) triggered by {single|pr}
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    print(template_reports.find_map_by_ids(id, report_id, report_trigger))


@main.command(name="find_report_maps")
@click.argument("template")
@click.option(
    "--report", "--report_id", default=None, type=str, help="The id of the report"
)
@click.option(
    "--report_trigger",
    default="",
    help="The event that will trigger the generation of the report. Can be either 'single' which means the report will "
    "be generated when a run successfully finishes, or 'pr' which means the report will be generated when a Github"
    " PR comparison run successfully finishes.",
    type=click.Choice(["single", "pr", ""], case_sensitive=False),
)
@click.option(
    "--created_before",
    default=None,
    type=str,
    help="Upper bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default=None,
    type=str,
    help="Lower bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default=None,
    type=str,
    help="Email of the creator of the map, case sensitive",
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
def find_report_maps(
    template,
    report,
    report_trigger,
    created_before,
    created_after,
    created_by,
    sort,
    limit,
    offset,
):
    """
    Retrieve the mapping record from the template specified by ID to the report specified by
    REPORT_ID
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for report
    if report:
        report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
            report, reports, "report_id", "report"
        )
    else:
        report_id = None
    if report_trigger == "":
        report_trigger = None
    print(
        template_reports.find_maps(
            id,
            report_id,
            report_trigger,
            created_before,
            created_after,
            created_by,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="delete_report_map_by_id")
@click.argument("template")
@click.argument("report")
@click.argument(
    "report_trigger", type=click.Choice(["single", "pr"], case_sensitive=False)
)
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of mapping created by "
    "another user",
)
def delete_report_map_by_id(template, report, report_trigger, yes):
    """
    Delete the mapping record from the template specified by TEMPLATE (id or name) to the report
    specified by REPORT (id or name) triggered by REPORT_TRIGGER, if the specified template has no non-failed (i.e.
    successful or currently running) runs associated with it
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(
        template, templates, "template_id", "template"
    )
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(
        report, reports, "report_id", "report"
    )
    # Unless user specifies --yes flag, check first to see if the record exists and prompt to user to confirm delete if
    # they are not the creator
    if not yes:
        # Try to find the record by id
        record = json.loads(
            template_reports.find_map_by_ids(id, report_id, report_trigger)
        )
        # If the returned record has a created_by field that does not match the user email, prompt the user to confirm
        # the delete
        user_email = config.load_var("email")
        if "created_by" in record and record["created_by"] != user_email:
            # If they decide not to delete, exit
            if not click.confirm(
                f"Mapping for template with id {id} and report with id {report_id} triggered by {report_trigger} "
                f"was created by {record['created_by']}. Are you sure you want to delete?"
            ):
                LOGGER.info("Okay, aborting delete operation")
                sys.exit(0)
    print(template_reports.delete_map_by_ids(id, report_id, report_trigger))
