import logging

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(test_id):
    """Submits a request to CARROT's tests find_by_id mapping"""
    return request_handler.find_by_id("tests", test_id)


def find(
    test_id="",
    template_id="",
    name="",
    description="",
    test_input_defaults="",
    test_option_defaults="",
    eval_input_defaults="",
    eval_option_defaults="",
    created_by="",
    created_before="",
    created_after="",
    sort="",
    limit="",
    offset="",
):
    """Submits a request to CARROT's tests find mapping"""
    # Create parameter list
    params = [
        ("test_id", test_id),
        ("template_id", template_id),
        ("name", name),
        ("description", description),
        ("test_input_defaults", test_input_defaults),
        ("test_option_defaults", test_option_defaults),
        ("eval_input_defaults", eval_input_defaults),
        ("eval_option_defaults", eval_option_defaults),
        ("created_by", created_by),
        ("created_before", created_before),
        ("created_after", created_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("tests", params)


def create(
    name,
    template_id,
    description,
    test_input_defaults,
    test_option_defaults,
    eval_input_defaults,
    eval_option_defaults,
    created_by
):
    """Submits a request to CARROT's tests create mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("template_id", template_id),
        ("description", description),
        ("test_input_defaults", test_input_defaults),
        ("test_option_defaults", test_option_defaults),
        ("eval_input_defaults", eval_input_defaults),
        ("eval_option_defaults", eval_option_defaults),
        ("created_by", created_by),
    ]
    return request_handler.create("tests", params)


def update(
    test_id,
    name,
    description,
    test_input_defaults,
    test_option_defaults,
    eval_input_defaults,
    eval_option_defaults
):
    """Submits a request to CARROT's tests update mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("description", description),
        ("test_input_defaults", test_input_defaults),
        ("test_option_defaults", test_option_defaults),
        ("eval_input_defaults", eval_input_defaults),
        ("eval_option_defaults", eval_option_defaults),
    ]
    return request_handler.update("tests", test_id, params)


def run(test_id, name, test_input, test_options, eval_input, eval_options, created_by):
    """Submits a request to CARROT's test run mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("test_input", test_input),
        ("test_options", test_options),
        ("eval_input", eval_input),
        ("eval_options", eval_options),
        ("created_by", created_by),
    ]
    return request_handler.run(test_id, params)


def delete(test_id):
    """Submits a request to CARROT's tests delete mapping"""
    return request_handler.delete("tests", test_id)


def subscribe(test_id, email):
    """Submits a request to CARROT's tests subscribe mapping"""
    return request_handler.subscribe("tests", test_id, email)


def unsubscribe(test_id, email):
    """Submits a request to CARROT's tests unsubscribe mapping"""
    return request_handler.unsubscribe("tests", test_id, email)
