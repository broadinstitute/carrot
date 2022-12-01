import json as json_lib
import logging
import os
import pprint
import sys
import urllib
from enum import Enum

import requests

from ..config import manager as config

LOGGER = logging.getLogger(__name__)

class ResponseFormat(Enum):
    """Expected response format for a request"""
    JSON = 1
    BYTES = 2
    TEXT = 3

def find_by_id(entity, id, params=None, expected_format=ResponseFormat.JSON):
    """
    Submits a request to the find_by_id mapping for the specified entity with the specified id
    Optionally, query params can also be provided
    """
    # Build request address and send
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}/{id}"
    # Filter out params that are not set
    if params:
        params = list(filter(lambda param: param[1] is not None, params))
    return send_request("GET", address, params=params, expected_format=expected_format)


def find(entity, params, expected_format=ResponseFormat.JSON):
    """Submits a request to the find mapping for the specified entity with the specified params"""
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}"
    # Filter out params that are not set
    params = list(filter(lambda param: param[1] is not None, params))
    # Create and send request
    return send_request("GET", address, params=params, expected_format=expected_format)


def create(entity, params, query_params=None, files=None):
    """
    Submits a request to create mapping for the specified entity with the specified params.
    If a value is specified for files, the request body will be multipart/form-data. If not, the
    request will have a json body
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}"
    # Build request body from params, filtering out empty ones
    body = {}
    for param in params:
        if param[1] is not None:
            body[param[0]] = param[1]
    # Build and send request
    # If we have files, send multipart
    if files:
        return send_request("POST", address, body=body, files=files)
    # Otherwise, send json
    else:
        return send_request("POST", address, json=body, params=query_params)


def update(entity, id, params, files=None):
    """
    Submits a request to update mapping for the specified entity with the specified id and
    params. If a value is specified for files, the request body will be multipart/form-data. If
    not, the request will have a json body
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}/{id}"
    # Build request json body from params, filtering out empty ones
    body = {}
    for param in params:
        if param[1] is not None:
            body[param[0]] = param[1]
    # Build and send request
    # If we have files, send multipart
    if files:
        return send_request("PUT", address, body=body, files=files)
    # Otherwise, send json
    else:
        return send_request("PUT", address, json=body)


def delete(entity, id):
    """
    Submits a request to the delete mapping for the specified entity with the specified id
    """
    # Build request address and send
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}/{id}"
    return send_request("DELETE", address)


def subscribe(entity, id, email):
    """
    Submits a request to the subscribe mapping for the specified entity with the specified id
    and email
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}/{id}/subscriptions"
    # Build request json body with email
    body = {"email": email}
    # Build and send request
    return send_request("POST", address, json=body)


def unsubscribe(entity, id, email):
    """
    Submits a request to the subscribe mapping for the specified entity with the specified id
    and email
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}/{id}/subscriptions"
    # Build request params with email
    params = [("email", email)]
    # Build and send request
    return send_request("DELETE", address, params=params)


def run(test_id, params):
    """
    Submits a POST request to the run mapping for the test with the specified id and params
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/tests/{test_id}/runs"
    # Build request json body from params, filtering out empty ones
    body = {}
    for param in params:
        if param[1] is not None:
            body[param[0]] = param[1]
    # Build and send request
    return send_request("POST", address, json=body)


def find_runs(entity, id, params, expected_format=ResponseFormat.JSON):
    """
    Submits a request to the find_runs mapping for the specified entity with the specified id
    and filtering by the specified params
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity}/{id}/runs"
    # Filter out params that are not set
    params = list(filter(lambda param: param[1] is not None, params))
    # Create and send request
    return send_request("GET", address, params=params, expected_format=expected_format)


def create_map(entity1, entity1_id, entity2, entity2_id, params, query_params=None):
    """
    Submits a request for creating a mapping between entity1 and entity2, with the specified
    params.
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = (
        f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}/{entity2_id}"
    )
    # Build request json body from params, filtering out empty ones
    body = {}
    for param in params:
        if param[1] is not None:
            body[param[0]] = param[1]
    # Create and send request
    return send_request("POST", address, json=body, params=query_params)


def find_map_by_ids(entity1, entity1_id, entity2, entity2_id):
    """
    Submits a request for finding a mapping between entity1 and entity2, with the specified
    ids.
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = (
        f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}/{entity2_id}"
    )
    # Create and send request
    return send_request("GET", address)


def find_maps(entity1, entity1_id, entity2, params):
    """
    Submits a request to the find_maps mapping for the specified entity with the specified id
    and filtering by the specified params
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}"
    # Filter out params that are not set
    params = list(filter(lambda param: param[1] is not None, params))
    # Create and send request
    return send_request("GET", address, params=params)


def delete_map_by_ids(entity1, entity1_id, entity2, entity2_id):
    """
    Submits a request for deleting a mapping between entity1 and entity2, with the specified
    ids.
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = (
        f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}/{entity2_id}"
    )
    # Create and send request
    return send_request("DELETE", address)

def create_map_with_target(entity1, entity1_id, entity2, entity2_id, target, params, query_params=None):
    """
    Submits a request for creating a mapping between entity1 and entity2 with the target, with the specified
    params.
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = (
        f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}/{entity2_id}/{target}"
    )
    # Build request json body from params, filtering out empty ones
    body = {}
    for param in params:
        if param[1] is not None:
            body[param[0]] = param[1]
    # Create and send request
    return send_request("POST", address, json=body, params=query_params)

def find_map_by_ids_and_target(entity1, entity1_id, entity2, entity2_id, target):
    """
    Submits a request for finding a mapping between entity1 and entity2, with the specified
    ids and target (like a report trigger, in the case of template reports)
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = (
        f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}/{entity2_id}/{target}"
    )
    # Create and send request
    return send_request("GET", address)


def delete_map_by_ids_and_target(entity1, entity1_id, entity2, entity2_id, target):
    """
    Submits a request for deleting a mapping between entity1 and entity2, with the specified
    ids and target (like a report trigger, in the case of template reports)
    """
    # Build request address
    server_address = config.load_var("carrot_server_address")
    address = (
        f"http://{server_address}/api/v1/{entity1}/{entity1_id}/{entity2}/{entity2_id}/{target}"
    )
    # Create and send request
    return send_request("DELETE", address)

def send_request(method, url, params=None, json=None, body=None, files=None, expected_format=ResponseFormat.JSON):
    """
    Sends a request to url with method, optionally with query params, json, form data body, and
    files, and handles potential errors. expected_format specifies the format we expect the
    response body to be in
    """
    processed_files = None
    try:
        # Convert files into the format we need to pass to requests
        processed_files = __process_file_dict(files)
        # Send request
        LOGGER.debug(
            "Sending %s request to %s with params %s and json %s and data %s and files %s",
            method,
            url,
            params,
            json,
            body,
            files
        )
        response = requests.request(method, url, params=params, json=json, data=body, files=processed_files)
        LOGGER.debug(
            "Received response with status %i and body %s",
            response.status_code,
            response.text,
        )
        # If the status code indicates an error, try to get and return the error message as json
        if response.status_code >= 300:
            json_body = response.json()
            if json_body is None:
                LOGGER.error(
                    "Received response with status %i and empty body" % response.status_code
                )
            else:
                LOGGER.error(json_lib.dumps(json_body, indent=4, sort_keys=True))
            sys.exit(1)
        # Get body in whatever format we expect
        if expected_format == ResponseFormat.JSON:
            # Parse json body from request and return
            json_body = response.json()
            if json_body is None:
                LOGGER.error(
                    "Received response with status %i and empty body" % response.status_code
                )
                sys.exit(1)
            return json_lib.dumps(json_body, indent=4, sort_keys=True)
        elif expected_format == ResponseFormat.BYTES:
            return response.content
        elif expected_format == ResponseFormat.TEXT:
            return response.text
    except (AttributeError, json_lib.decoder.JSONDecodeError):
        LOGGER.error("Failed to parse json from response body: %s", response.text)
        sys.exit(1)
    except requests.ConnectionError as err:
        LOGGER.debug(err)
        if LOGGER.getEffectiveLevel() == logging.DEBUG:
            LOGGER.error("Encountered a connection error.")
        else:
            LOGGER.error("Encountered a connection error. Enable verbose logging (-v) for more info")
        sys.exit(1)
    except requests.URLRequired as err:
        LOGGER.debug(err)
        if LOGGER.getEffectiveLevel() == logging.DEBUG:
            LOGGER.error("Invalid URL.")
        else:
            LOGGER.error("Invalid URL. Enable verbose logging (-v) for more info")
        sys.exit(1)
    except requests.Timeout as err:
        LOGGER.debug(err)
        if LOGGER.getEffectiveLevel() == logging.DEBUG:
            LOGGER.error("Request timed out.")
        else:
            LOGGER.error("Request timed out. Enable verbose logging (-v) for more info")
        sys.exit(1)
    except requests.TooManyRedirects as err:
        LOGGER.debug(err)
        if LOGGER.getEffectiveLevel() == logging.DEBUG:
            LOGGER.error("Too many redirects")
        else:
            LOGGER.error("Too many redirects. Enable verbose logging (-v) for more info")
        sys.exit(1)
    except IOError as err:
        LOGGER.debug(err)
        if LOGGER.getEffectiveLevel() == logging.DEBUG:
            LOGGER.error("Encountered an IO error")
        else:
            LOGGER.error("Encountered an IO error. Enable verbose logging (-v) for more info")
        sys.exit(1)
    finally:
        # Close any open files
        if processed_files is not None:
            __close_files(processed_files)

def __process_file_dict(files):
    """
    Accepts a dict of file params mapped to file paths and returns a dict formatted for passing
    files to requests

    Parameters
    ----------
    files - A dict mapping file param names to file paths

    Returns
    -------
    A dict mapping file param names to 2-tuples containing the file's basename and a file object
    for the open file
    """
    if files is None:
        return None
    # Dict we'll return containing the files
    processed_files = {}
    # Loop through the values in files, open each file, and add it to processed files with its filename
    for param_name in files:
        file_path = files[param_name]
        try:
            file = open(file_path, "rb")
            processed_files[param_name] = (os.path.basename(file.name), file)
        except IOError as e:
            LOGGER.error(f"Failed to open {param_name} file with path {file_path}.")
            # Close all the open files
            __close_files(processed_files)
            raise e
    return processed_files

def __close_files(files):
    """
    Closes all the files in files

    Parameters
    ----------
    files - A files dict formatted to match requests' files param (keys are the name of the param,
            mapped to tuples where the second value is a file object)

    Returns
    -------
    None
    """
    if files is None:
        return
    for param_name in files:
        files[param_name][1].close()
