import logging
import sys

LOGGER = logging.getLogger(__name__)


def get_software_version_query(
    software_name,
    commit_or_tag,
    commit_count,
    commit_from,
    commit_to,
    software_branch,
    tags_only=False,
):
    """
    Accepts parameters for filtering runs by software version and attempts to convert them into one of the expected
    variations for a software version query in the carrot rest api.  Returns a dict with the appropriate data or logs
    an error and exits if the provided parameters do not match one of the accepted options in carrot
    :param software_name: the name of the software in carrot
    :param commit_or_tag: a list of commits and/or tags to the software (or None)
    :param commit_count: a count of commits to the software
    :param commit_from: lower bound of date range for commits to the software
    :param commit_to: upper bound of date range for commits to the software
    :param software_branch: a branch on which to check for the commits
    :param tags_only: if true, specifies that the commit_count commits should be the last commit_count tags instead
    :return: a software version query dict in the form expected by the carrot api, or None if all params are None
    """
    # If it's a list
    if (
        software_name
        and commit_or_tag
        and not (
            commit_count or commit_from or commit_to or software_branch or tags_only
        )
    ):
        # We call list() on commit_or_tag here because it will come to us as a tuple
        return {"name": software_name, "commits_and_tags": list(commit_or_tag)}
    # If it's a count
    elif (
        software_name
        and commit_count
        and not (commit_or_tag or commit_to or commit_from)
    ):
        params = {"name": software_name, "count": commit_count, "tags_only": tags_only}
        if software_branch:
            params["branch"] = software_branch
        return params
    # If it's a date range
    elif (
        software_name
        and (commit_to or commit_from)
        and not (commit_count or commit_or_tag or tags_only)
    ):
        params = {"name": software_name}
        if commit_to:
            params["to"] = commit_to
        if commit_from:
            params["from"] = commit_from
        if software_branch:
            params["branch"] = software_branch
        return params
    # If none are provided
    if not (
        software_name
        or commit_or_tag
        or commit_count
        or commit_to
        or commit_from
        or software_branch
    ):
        return None
    # Otherwise, it's invalid
    provided_params = []
    if software_name:
        provided_params.append("--software_name")
    if commit_or_tag:
        provided_params.append("--commit_or_tag")
    if commit_count:
        provided_params.append("--commit_count")
    if commit_from:
        provided_params.append("--commit_from")
    if commit_to:
        provided_params.append("--commit_to")
    if software_branch:
        provided_params.append("--software_branch")
    if tags_only:
        provided_params.append("--tags_only")
    LOGGER.error(
        "Invalid combination of parameters for filtering by software version.  There are three acceptable "
        "combinations of parameters:\n"
        "Commits/tags list: --software_name and one or more --commit_or_tag\n"
        "Commit count: --software_name, --commit_count, and optionally --software_branch and/or --tags_only\n"
        "Date range: --software_name, --commit_from and/or --commit_to, and optionally --software_branch\n"
        "The provided combination of params is not allowed: "
        + ", ".join(provided_params)
    )
    sys.exit(1)
