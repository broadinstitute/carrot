import logging
import re
import sys

from .config import manager as config

LOGGER = logging.getLogger(__name__)

def verify_email(email_maybe):
    """
    Returns True if the email matches the format .*@.*\..* and False if not
    """
    return bool(re.match(r".*@.*\..*", email_maybe))

def check_created_by(created_by):
    """
    Checks created_by to see if it has a value.  If not, attempts to return the value for the email config variable. If
    there is no value for the email config variable, prints an error and exits.  If created_by has a value, verifies it
    is an email and returns it if so or prints an error and exits if not
    """
    if created_by is None:
        email_config_val = config.load_var_no_error("email")
        if email_config_val is not None:
            return email_config_val
        else:
            LOGGER.error(
                "No email config variable set.  If a value is not specified for --created_by, "
                "there must be a value set for email."
            )
            sys.exit(1)
    else:
        if verify_email(created_by):
            return created_by
        else:
            LOGGER.error("Value provided for --created_by is not a valid email address")
            sys.exit(1)
