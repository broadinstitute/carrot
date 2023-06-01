import logging
import sys

import click

from .. import command_util
from .. import dependency_util
from .. import file_util
from ..config import manager as config
from ..rest import pipelines, runs

LOGGER = logging.getLogger(__name__)


@click.group(name="helper")
def main():
    """A utility script for automatically populating carrot resources from the directory structure of a given path."""
