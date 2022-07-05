import logging
from sys import argv

import click

from .config import command as config
from .config import manager as config_manager
from .pipeline import command as pipeline
from .report import command as report
from .result import command as result
from .run import command as run
from .run_group import command as run_group
from .software import command as software
from .subscription import command as subscription
from .template import command as template
from .test import command as test

# Version number is automatically set via bumpversion.
# DO NOT MODIFY:
__version = "0.5.1"

# Create a logger for this module:
LOGGER = logging.getLogger(__name__)

# Context settings for commands, for overwriting some click defaults
CONTEXT_SETTINGS = dict(help_option_names=['-h', '--help'])


@click.group(
    name="carrot_cli",
    context_settings=CONTEXT_SETTINGS
)
@click.option(
    "-q",
    "--quiet",
    "verbosity",
    flag_value=logging.CRITICAL + 10,
    help="Suppress all logging",
)
@click.option(
    "-v",
    "--verbose",
    "verbosity",
    flag_value=logging.DEBUG,
    help="More verbose logging",
)
@click.option(
    "--trace",
    "verbosity",
    flag_value=logging.NOTSET,
    help="Highest level logging for debugging",
)
def main_entry(verbosity):
    # Set up our log verbosity
    from . import log  # pylint: disable=C0415

    log.configure_logging(verbosity)

    # Make sure we have a config file
    config_manager.create_config_dir_if_not_exists()

    # Log our command-line and log level so we can have it in the log file:
    LOGGER.info("Invoked by: %s", " ".join(argv))
    LOGGER.info("Log level set to: %s", logging.getLevelName(logging.getLogger().level))


@main_entry.command()
def version():
    """Print the version of carrot_cli"""
    LOGGER.info("carrot_cli %s", __version__)


# Update with new sub-commands:
main_entry.add_command(pipeline.main)
main_entry.add_command(template.main)
main_entry.add_command(test.main)
main_entry.add_command(subscription.main)
main_entry.add_command(result.main)
main_entry.add_command(run.main)
main_entry.add_command(run_group.main)
main_entry.add_command(software.main)
main_entry.add_command(config.main)
main_entry.add_command(report.main)

if __name__ == "__main__":
    main_entry()  # pylint: disable=E1120
