import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(run_group_id):
    """
    Submits a request to CARROT's run-groups find_by_id mapping
    """
    return request_handler.find_by_id("run-groups", run_group_id)


def find(
    run_group_id="",
    owner="",
    repo="",
    issue_number="",
    author="",
    base_commit="",
    head_commit="",
    test_input_key="",
    eval_input_key="",
    created_before="",
    created_after="",
    sort="",
    limit="",
    offset="",
):
    """
    Submits a request to CARROT's find run-groups mapping filtering by the specified parameters
    """
    # Create parameter list
    params = [
        ("run_group_id", run_group_id),
        ("owner", owner),
        ("repo", repo),
        ("issue_number", issue_number),
        ("author", author),
        ("base_commit", base_commit),
        ("head_commit", head_commit),
        ("test_input_key", test_input_key),
        ("eval_input_key", eval_input_key),
        ("created_before", created_before),
        ("created_after", created_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("run-groups", params)


def delete(run_group_id):
    """Submits a request to CARROT's run-groups delete mapping"""
    return request_handler.delete("run-groups", run_group_id)
