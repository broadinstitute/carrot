import logging

import click

from . import manager

LOGGER = logging.getLogger(__name__)


@click.group(name="config")
def main():
    """Commands for setting and displaying config variables"""
    pass


@main.command(name="set")
@click.argument("variable")
@click.argument("value")
def set_var(variable, value):
    """
    Set the value of a specified config variable

    \b
    Config variables:
    carrot_server_address
        The address of the CARROT server you'd like to connect to
    email
        Your email address, for provenance and notification purposes
    """
    # If the user tries to set a variable that isn't a valid config variable, print a message
    if variable not in manager.CONFIG_VARIABLES:
        allowed_variables = ", ".join(manager.CONFIG_VARIABLES)
        print(
            f"{variable} is not a config variable. "
            f"The config variables that can be set are: {allowed_variables}"
        )
    # Otherwise, set the variable
    else:
        manager.set_var(variable, value)
        print("Success!")


@main.command(name="get")
def get_config():
    """Prints the current config"""
    print(manager.get_config())
