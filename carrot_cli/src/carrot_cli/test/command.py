import json
import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from .. import file_util
from ..config import manager as config
from ..rest import runs, templates, tests

LOGGER = logging.getLogger(__name__)


@click.group(name="test")
def main():
    """Commands for searching, creating, and updating tests"""


@main.command(name="find_by_id")
@click.argument("id")
def find_by_id(id):
    """Retrieve a test by its ID"""
    print(tests.find_by_id(id))


@main.command(name="find")
@click.option("--test_id", default="", help="The test's ID")
@click.option(
    "--template",
    "--template_id",
    "--template_name",
    default="",
    help="The ID or name of the template that is the test's parent",
)
@click.option("--name", default="", help="The name of the test, case-sensitive")
@click.option(
    "--description", default="", help="The description of the test, case-sensitive"
)
@click.option(
    "--test_input_defaults",
    default="",
    help="A JSON file containing the default inputs to the test WDL for the test",
)
@click.option(
    "--test_option_defaults",
    default="",
    help="A JSON file containing the default workflow options for the test WDL for the test",
)
@click.option(
    "--eval_input_defaults",
    default="",
    help="A JSON file containing the default inputs to the eval WDL for the test",
)
@click.option(
    "--eval_option_defaults",
    default="",
    help="A JSON file containing the default workflow options for the eval WDL for the test",
)
@click.option(
    "--created_before",
    default="",
    help="Upper bound for test's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_after",
    default="",
    help="Lower bound for test's created_at value, in the format YYYY-MM-DDThh:mm:ss.ssssss",
)
@click.option(
    "--created_by", default="", help="Email of the creator of the test, case sensitive"
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
    help="The maximum number of test records to return",
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
    test_id,
    template,
    name,
    description,
    test_input_defaults,
    test_option_defaults,
    eval_input_defaults,
    eval_option_defaults,
    created_by,
    created_before,
    created_after,
    sort,
    limit,
    offset,
):
    """Retrieve tests filtered to match the specified parameters"""
    # Process template in case it's a name
    if template:
        template_id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    else:
        template_id = ""
    # Load data from files for test_input_defaults, test_option_defaults, eval_input_defaults and eval_option_defaults,
    # if set
    test_input_defaults = file_util.read_file_to_json(test_input_defaults)
    eval_input_defaults = file_util.read_file_to_json(eval_input_defaults)
    test_option_defaults = file_util.read_file_to_json(test_option_defaults)
    eval_option_defaults = file_util.read_file_to_json(eval_option_defaults)

    print(
        tests.find(
            test_id,
            template_id,
            name,
            description,
            test_input_defaults,
            test_option_defaults,
            eval_input_defaults,
            eval_option_defaults,
            created_by,
            created_before,
            created_after,
            sort,
            limit,
            offset,
        )
    )


@main.command(name="create")
@click.option("--name", help="The name of the test", required=True)
@click.option(
    "--template",
    "--template_id",
    required=True,
    help="The ID or name of the template that will be the test's parent",
)
@click.option("--description", default="", help="The description of the test")
@click.option(
    "--test_input_defaults",
    default="",
    help="A JSON file containing the default inputs to the test WDL for the test",
)
@click.option(
    "--test_option_defaults",
    default="",
    help="A JSON file containing the default workflow options for the test WDL for the test",
)
@click.option(
    "--eval_input_defaults",
    default="",
    help="A JSON file containing the default inputs to the eval WDL for the test",
)
@click.option(
    "--eval_option_defaults",
    default="",
    help="A JSON file containing the default workflow options for the eval WDL for the test",
)
@click.option(
    "--created_by",
    default="",
    help="Email of the creator of the test.  Defaults to email config variable",
)
def create(
    name,
    template,
    description,
    test_input_defaults,
    test_option_defaults,
    eval_input_defaults,
    eval_option_defaults,
    created_by
):
    """Create test with the specified parameters"""
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
    template_id = dependency_util.get_id_from_id_or_name_and_handle_error(template, templates, "template_id", "template")
    # Load data from files for test_input_defaults, test_option_defaults, eval_input_defaults and eval_option_defaults,
    # if set
    test_input_defaults = file_util.read_file_to_json(test_input_defaults)
    eval_input_defaults = file_util.read_file_to_json(eval_input_defaults)
    test_option_defaults = file_util.read_file_to_json(test_option_defaults)
    eval_option_defaults = file_util.read_file_to_json(eval_option_defaults)
    print(
        tests.create(
            name,
            template_id,
            description,
            test_input_defaults,
            test_option_defaults,
            eval_input_defaults,
            eval_option_defaults,
            created_by,
        )
    )


@main.command(name="update")
@click.argument("test")
@click.option("--name", default="", help="The name of the test")
@click.option("--description", default="", help="The description of the test")
@click.option(
    "--test_input_defaults",
    default="",
    help="A JSON file containing the default inputs to the test WDL for the test. Updating this "
    "parameter is allowed only if the specified test has no non-failed (i.e. successful or "
    "currently running) runs associated with it",
)
@click.option(
    "--test_option_defaults",
    default="",
    help="A JSON file containing the default workflow options for the test WDL for the test. Updating this "
    "parameter is allowed only if the specified test has no non-failed (i.e. successful or "
    "currently running) runs associated with it",
)
@click.option(
    "--eval_input_defaults",
    default="",
    help="A JSON file containing the default inputs to the eval WDL for the test. Updating this "
    "parameter is allowed only if the specified test has no non-failed (i.e. successful or "
    "currently running) runs associated with it",
)
@click.option(
    "--eval_option_defaults",
    default="",
    help="A JSON file containing the default workflow options for the eval WDL for the test. Updating this "
    "parameter is allowed only if the specified test has no non-failed (i.e. successful or "
    "currently running) runs associated with it",
)
def update(test, name, description, test_input_defaults, test_option_defaults, eval_input_defaults, eval_option_defaults):
    """Update test for TEST (id or name) with the specified parameters"""
    # Process test to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(test, tests, "test_id", "test")
    # Load data from files for test_input_defaults, test_option_defaults, eval_input_defaults and eval_option_defaults,
    # if set
    test_input_defaults = file_util.read_file_to_json(test_input_defaults)
    eval_input_defaults = file_util.read_file_to_json(eval_input_defaults)
    test_option_defaults = file_util.read_file_to_json(test_option_defaults)
    eval_option_defaults = file_util.read_file_to_json(eval_option_defaults)
    print(tests.update(
        id, name, description, test_input_defaults, test_option_defaults, eval_input_defaults, eval_option_defaults
    ))


@main.command(name="delete")
@click.argument("test")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    default=False,
    help="Automatically answers yes if prompted to confirm delete of test created by "
    "another user",
)
def delete(test, yes):
    """Delete a test by its ID or name, if the test has no runs associated with it"""
    # Process test to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(test, tests, "test_id", "test")
    command_util.delete(id, yes, tests, "Test")


@main.command(name="run")
@click.argument("test")
@click.option(
    "--name",
    default="",
    help="The name of the run.  Will be autogenerated if not specified",
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
    "--created_by",
    default="",
    help="Email of the creator of the run.  Defaults to email config variable",
)
def run(test, name, test_input, test_options, eval_input, eval_options, created_by):
    """Start a run for the test specified by TEST (id or name) with the specified params"""
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
    # Process test to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(test, tests, "test_id", "test")
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)
    print(tests.run(id, name, test_input, test_options, eval_input, eval_options, created_by))


@main.command(name="find_runs")
@click.argument("test")
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
    test,
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
    """Retrieve runs of the test specified by TEST (id or name), filtered by the specified parameters"""
    # Process test to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(test, tests, "test_id", "test")
    # Load data from files for test_input, test_options, eval_input and eval_options, if set
    test_input = file_util.read_file_to_json(test_input)
    test_options = file_util.read_file_to_json(test_options)
    eval_input = file_util.read_file_to_json(eval_input)
    eval_options = file_util.read_file_to_json(eval_options)
    print(
        runs.find(
            "tests",
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
@click.argument("test")
@click.option(
    "--email",
    default="",
    help="The email address to receive notifications. If set, takes priority over email config "
    "variable",
)
def subscribe(test, email):
    """Subscribe to receive notifications about the test specified by TEST (id or name)"""
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
    # Process test to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(test, tests, "test_id", "test")
    print(tests.subscribe(id, email))


@main.command(name="unsubscribe")
@click.argument("test")
@click.option(
    "--email",
    default="",
    help="The email address to stop receiving notifications. If set, takes priority over email "
    "config variable",
)
def unsubscribe(test, email):
    """Delete subscription to the test with the specified by TEST (id or name) and email"""
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
    # Process test to get id if it's a name
    id = dependency_util.get_id_from_id_or_name_and_handle_error(test, tests, "test_id", "test")
    print(tests.unsubscribe(id, email))
