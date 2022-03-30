import json
import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from .. import file_util
from ..config import manager as config
from ..rest import pipelines, reports, results, runs, template_reports, template_results, templates

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
@click.option("--template_id", default="", help="The template's ID")
@click.option(
    "--pipeline",
    "--pipeline_id",
    "--pipeline_name",
    default="",
    help="The ID or name of the pipeline that is the template's parent",
)
@click.option("--name", default="", help="The name of the template, case-sensitive")
@click.option(
    "--description", default="", help="The description of the template, case-sensitive"
)
@click.option(
    "--test_wdl",
    default="",
    help="The location where the test WDL for the template is hosted, either in the form of a "
    "http/https/gs uri",
)
@click.option(
    "--eval_wdl",
    default="",
    help="The location where the eval WDL for the template is hosted, either in the form of a "
    "http/https/gs uri",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for template's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for template's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the template, case sensitive",
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
        pipeline_id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")
    else:
        pipeline_id = ""
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
    required=True,
)
@click.option("--name", help="The name of the template", required=True)
@click.option("--description", default="", help="The description of the template")
@click.option(
    "--test_wdl",
    required=True,
    help="The location where the test WDL for this template is hosted, or its local file path. The"
    "test WDL is the WDL which defines the thing the be tested",
)
@click.option(
    "--test_wdl_dependencies",
    help="The location where the test WDL dependencies zip for this template is hosted, or its"
    "local file path. The zip should be formatted the same as it would be for cromwell",
)
@click.option(
    "--eval_wdl",
    required=True,
    help="The location where the eval WDL for ths template is hosted.  The eval WDL is the WDL "
    "which takes the outputs from the test WDL and evaluates them",
)
@click.option(
    "--eval_wdl_dependencies",
    help="The location where the eval WDL dependencies zip for this template is hosted, or its"
    "local file path. The zip should be formatted the same as it would be for cromwell",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the template.  Defaults to email config variable",
)
def create(
    name,
    pipeline,
    description,
    test_wdl,
    test_wdl_dependencies,
    eval_wdl,
    eval_wdl_dependencies,
    created_by
):
    """Create template with the specified parameters"""
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
    # Process pipeline to get id if it's a name
    pipeline_id = dependency_util.get_id_from_id_or_name_and_handle_error(pipeline, pipelines, "pipeline_id", "pipeline")

    print(
        templates.create(
            name,
            pipeline_id,
            description,
            test_wdl,
            test_wdl_dependencies,
            eval_wdl,
            eval_wdl_dependencies,
            created_by
        )
    )


@main.command(name="update")
@click.argument("template")
@click.option("--name", default="", help="The new name of the template")
@click.option("--description", default="", help="The new description of the template")
@click.option(
    "--test_wdl",
    default="",
    help="The location where the new test WDL for the template is hosted or a local file path.  "
    "Updating this parameter is allowed only if the specified template has no non-failed "
    "(i.e. successful or currently running) runs associated with it",
)
@click.option(
    "--test_wdl_dependencies",
    default="",
    help="The location where the new test WDL dependencies zip for the template is hosted or a "
    "local file path.  Updating this parameter is allowed only if the specified template has no "
    "non-failed (i.e. successful or currently running) runs associated with it",
)
@click.option(
    "--eval_wdl",
    default="",
    help="The location where the new eval WDL for the template is hosted or a local file path.  "
    "Updating this parameter is allowed only if the specified template has no non-failed (i.e. "
    "successful or currently running) runs associated with it",
)
@click.option(
    "--eval_wdl_dependencies",
    default="",
    help="The location where the new eval WDL dependencies zip for the template is hosted or a "
    "local file path.  Updating this parameter is allowed only if the specified template has no "
    "non-failed (i.e. successful or currently running) runs associated with it",
)
def update(template, name, description, test_wdl, test_wdl_dependencies, eval_wdl, eval_wdl_dependencies):
    """Update template with TEMPLATE (id or name) with the specified parameters"""
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")

    print(templates.update(id, name, description, test_wdl, test_wdl_dependencies, eval_wdl, eval_wdl_dependencies))



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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")

    command_util.delete(id, yes, templates, "Template")


@main.command(name="find_runs")
@click.argument("template")
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
    template,
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
    Retrieve runs related to the template specified by TEMPLATE (id or name), filtered by the
    specified parameters
    """
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)

    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")

    print(
        runs.find(
            "templates",
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
@click.argument("template")
@click.option(
    "--email",
    default="",
    help="The email address to receive notifications. If set, takes priority over email config "
    "variable",
)
def subscribe(template, email):
    """Subscribe to receive notifications about the template specified by TEMPLATE (id or name)"""
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
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")

    print(templates.subscribe(id, email))


@main.command(name="unsubscribe")
@click.argument("template")
@click.option(
    "--email",
    default="",
    help="The email address to stop receiving notifications. If set, takes priority over email "
    "config variable",
)
def unsubscribe(template, email):
    """Delete subscription to the template specified by TEMPLATE (id or name) and email"""
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
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")

    print(templates.unsubscribe(id, email))


@main.command(name="map_to_result")
@click.argument("template")
@click.argument("result")
@click.argument("result_key")
@click.option("--created_by", default="", help="Email of the creator of the mapping")
def map_to_result(template, result, result_key, created_by):
    """
    Map the template specified by TEMPLATE (id or name) to the result specified by RESULT (id or
    name) for RESULT_KEY in in the output generated by that template
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
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for result
    result_id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for result
    result_id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
    print(template_results.find_map_by_ids(id, result_id))


@main.command(name="find_result_maps")
@click.argument("template")
@click.option("--result", "--result_id", default="", help="The id or name of the result")
@click.option(
    "--result_key",
    default="",
    help="The key used to name the result within the output of the template",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by", default="", help="Email of the creator of the map, case sensitive"
)
@click.option(
    "--sort",
    default="",
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for result
    if result:
        result_id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
    else:
        result_id = ""
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for result
    result_id = dependency_util.get_id_from_id_or_name_and_handle_error(result, results, "result_id", "result")
    command_util.delete_map(id, result_id, yes, template_results, "template", "result")


@main.command(name="map_to_report")
@click.argument("template")
@click.argument("report")
@click.option("--created_by", default="", help="Email of the creator of the mapping")
def map_to_report(template, report, created_by):
    """
    Map the template specified by TEMPLATE (id or name) to the report specified by REPORT (id or
    name)
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
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    print(template_reports.create_map(id, report_id, created_by))


@main.command(name="find_report_map_by_id")
@click.argument("template")
@click.argument("report")
def find_report_map_by_id(template, report):
    """
    Retrieve the mapping record from the template specified by TEMPLATE (id or name) to the report
    specified by REPORT (id or name)
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    print(template_reports.find_map_by_ids(id, report_id))


@main.command(name="find_report_maps")
@click.argument("template")
@click.option("--report", "--report_id", default="", help="The id of the report")
@click.option(
    "--created_before",
    default="",
    help="Upper bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for map's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by", default="", help="Email of the creator of the map, case sensitive"
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
def find_report_maps(
    template,
    report,
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
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for report
    if report:
        report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    else:
        report_id = ""
    print(
        template_reports.find_maps(
            id,
            report_id,
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
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of mapping created by "
    "another user",
)
def delete_report_map_by_id(template, report, yes):
    """
    Delete the mapping record from the template specified by TEMPLATE (id or name) to the report
    specified by REPORT (id or name), if the specified template has no non-failed (i.e. successful
    or currently running) runs associated with it
    """
    # Process template to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Same for report
    report_id = dependency_util.get_id_from_id_or_name_and_handle_error(report, reports, "report_id", "report")
    command_util.delete_map(id, report_id, yes, template_reports, "template", "report")
