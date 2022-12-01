import json
import logging
import sys

LOGGER = logging.getLogger(__name__)


def read_file_to_json(filename):
    """
    Opens the file specified by filename to read and returns its contents parsed as JSON if
    successful, empty string if filename is empty, or exits if it fails
    """
    if filename is not None:
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
        return None

def write_data_to_file(data, filename):
    """
    Writes data to the file specified by filename
    :param data: bytes to write to file
    :param filename: filename to write to
    """
    try:
        with open(filename, "w+b") as data_file:
            data_file.write(data)
    except IOError as e:
        LOGGER.error(
            f"Encountered the following error while trying to write data to file {filename}: {e}"
        )
        sys.exit(1)

