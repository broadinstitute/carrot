import json
import logging
import os
import sys

LOGGER = logging.getLogger(__name__)

CONFIG_VARIABLES = ["carrot_server_address", "email"]

__CURRENT_CONFIG = {}


def create_config_dir_if_not_exists():
    """Creates the .carrot_cli dir and config file if they don't exist"""
    config_dir_path = os.path.expanduser("~/.carrot_cli")
    if not os.path.exists(config_dir_path):
        LOGGER.info("Config directory not found. Creating at %s", config_dir_path)
        try:
            os.makedirs(config_dir_path)
        except OSError:
            LOGGER.error("Failed to create config directory at %s", config_dir_path)
            sys.exit(1)
    config_file_path = os.path.expanduser("~/.carrot_cli/config.json")
    if not os.path.exists(config_file_path):
        LOGGER.info("Config file not found. Creating at %s", config_file_path)
        try:
            with open(config_file_path, "w") as config_file:
                json.dump({}, config_file)
        except OSError:
            LOGGER.error("Failed to created config file at %s", config_file_path)
            sys.exit(1)


def load_var(var_name):
    """Returns specified variable from config file, or prints message and exits if not set"""
    value = load_var_no_error(var_name)
    if value is None:
        # If the value isn't set, print an error
        print(
            f"Config variable {var_name} not set. "
            "Please set the variable using: carrot_cli config set"
        )
        sys.exit(1)
    else:
        # If it is, return it
        return value


def load_var_no_error(var_name):
    """Returns specified variable from config file, or None if not set"""
    # Open file and load as json
    config_file_path = os.path.expanduser("~/.carrot_cli/config.json")
    with open(config_file_path, "r") as config_file:
        LOGGER.debug("Loading config variable %s", var_name)
        config_json = json.load(config_file)
        # Return variable value if it's set
        if var_name in config_json:
            return config_json[var_name]
        LOGGER.debug("Config file did not contain variable %s", var_name)
        return None


def set_var(var_name, val):
    """Sets the specified variable with the specified value in the config file"""
    LOGGER.debug("Setting config variable %s to %s", var_name, val)
    # Open file and load as json
    config_file_path = os.path.expanduser("~/.carrot_cli/config.json")
    config_file = open(config_file_path, "r+")
    config_json = json.load(config_file)
    config_file.seek(0)
    # Set var
    config_json[var_name] = val
    # Write back to file
    json.dump(config_json, config_file, sort_keys=True, indent=4, ensure_ascii=False)
    config_file.truncate()


def get_config():
    # Open file and load as json
    config_file_path = os.path.expanduser("~/.carrot_cli/config.json")
    with open(config_file_path, "r") as config_file:
        return json.dumps(json.load(config_file), indent=4, sort_keys=True)
