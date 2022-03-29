import json
import logging
import sys

LOGGER = logging.getLogger(__name__)


def read_file_to_json(filename):
    """
    Opens the file specified by filename to read and returns its contents parsed as JSON if
    successful, empty string if filename is empty, or exits if it fails
    """
    if filename != "":
        try:
            with open(filename, "r") as input_file:
                return json.load(input_file)
        except FileNotFoundError:
            LOGGER.error(
                "Encountered FileNotFound error when trying to read %s",
                filename,
            )
            sys.exit(1)
        except json.JSONDecodeError:
            LOGGER.error(
                "Encountered JSONDecodeError error when trying to read %s",
                filename,
            )
            sys.exit(1)
    else:
        return ""
