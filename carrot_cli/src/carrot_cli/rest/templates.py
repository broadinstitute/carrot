import logging
import os

from . import request_handler

LOGGER = logging.getLogger(__name__)


def find_by_id(template_id):
    """Submits a request to CARROT's templates find_by_id mapping"""
    return request_handler.find_by_id("templates", template_id)


def find(
    template_id="",
    pipeline_id="",
    name="",
    description="",
    test_wdl="",
    eval_wdl="",
    created_by="",
    created_before="",
    created_after="",
    sort="",
    limit="",
    offset="",
):
    """Submits a request to CARROT's templates find mapping"""
    # Create parameter list
    params = [
        ("template_id", template_id),
        ("pipeline_id", pipeline_id),
        ("name", name),
        ("description", description),
        ("test_wdl", test_wdl),
        ("eval_wdl", eval_wdl),
        ("created_by", created_by),
        ("created_before", created_before),
        ("created_after", created_after),
        ("sort", sort),
        ("limit", limit),
        ("offset", offset),
    ]
    return request_handler.find("templates", params)


def create(
    name,
    pipeline_id,
    description,
    test_wdl,
    test_wdl_dependencies,
    eval_wdl,
    eval_wdl_dependencies,
    created_by
):
    """Submits a request to CARROT's templates create mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("pipeline_id", pipeline_id),
        ("description", description),
        ("created_by", created_by),
    ]
    # Start files as an empty dict
    files = {}
    # Process test and eval wdls and dependencies to put them in the correct lists depending on
    # how they are provided
    __process_maybe_file_field(params, files, "test_wdl", test_wdl)
    if test_wdl_dependencies:
        __process_maybe_file_field(params, files, "test_wdl_dependencies", test_wdl_dependencies)
    __process_maybe_file_field(params, files, "eval_wdl", eval_wdl)
    if eval_wdl_dependencies:
        __process_maybe_file_field(params, files, "eval_wdl_dependencies", eval_wdl_dependencies)
    # Make the request
    return request_handler.create("templates", params, files=(files if files else None))


def update(
    template_id,
    name,
    description,
    test_wdl,
    test_wdl_dependencies,
    eval_wdl,
    eval_wdl_dependencies
):
    """Submits a request to CARROT's templates update mapping"""
    # Create parameter list
    params = [
        ("name", name),
        ("description", description),
    ]
    # Start files as an empty dict
    files = {}
    # Process test and eval wdls and dependencies (if provided) to put them in the correct lists
    # depending on how they are provided
    if test_wdl:
        __process_maybe_file_field(params, files, "test_wdl", test_wdl)
    if test_wdl_dependencies:
        __process_maybe_file_field(params, files, "test_wdl_dependencies", test_wdl_dependencies)
    if eval_wdl:
        __process_maybe_file_field(params, files, "eval_wdl", eval_wdl)
    if eval_wdl_dependencies:
        __process_maybe_file_field(params, files, "eval_wdl_dependencies", eval_wdl_dependencies)
    # Make the request
    return request_handler.update("templates", template_id, params, files=(files if files else None))


def delete(template_id):
    """Submits a request to CARROT's templates delete mapping"""
    return request_handler.delete("templates", template_id)


def subscribe(template_id, email):
    """Submits a request to CARROT's templates subscribe mapping"""
    return request_handler.subscribe("templates", template_id, email)


def unsubscribe(template_id, email):
    """Submits a request to CARROT's templates unsubscribe mapping"""
    return request_handler.unsubscribe("templates", template_id, email)


def __process_maybe_file_field(params, files, field_name, field_val):
    """
    Accepts the name and value for a field that is either a file or an http/https/gs uri and adds
    it to either the files dict or params list respectively

    Parameters
    ----------
    params - In-progress params list for a request
    files - In-progress files dict for a request (keys are param names and vals are file paths)
    field_name - The name of the field, if it were added to the params list (it will be appended
                with _file if added to the files dict
    field_val - The value of the field: either an http/https/gs uri or a local file path

    Returns
    -------
    None
    """
    # If field_val is an http or gs uri, we'll add it to params
    if field_val.startswith("http://") \
            or field_val.startswith("https://") \
            or field_val.startswith("gs://"):
        params.append((field_name, field_val))
    # Otherwise, assume field_val is a file, so we'll throw it in the files list
    else:
        files[f'{field_name}_file'] = field_val