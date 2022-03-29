import json
import logging
import sys
import uuid

LOGGER = logging.getLogger(__name__)

def get_id_from_id_or_name_and_handle_error(id_or_name, module, id_key, entity_name):
    """
    Convenience wrapper function for get_id_from_id_or_name that prints an error message and exits
    in the case of an exception

    Parameters
    ----------
    id_or_name - a string containing either a UUID or a name of a record
    module - the rest module corresponding to the type of record; must define a find function
    id_key - the key for the id in the record that would be returned
    entity_name - the name of the entity we're checking an id for, for printing in the error message

    Returns
    -------

    """
    try:
        # Attempt to process id_or_name and return
         return get_id_from_id_or_name(id_or_name, module, id_key)
    except RecordNotFoundError as e:
        LOGGER.debug(
            f"Encountered RecordNotFoundError when running get_id_from_id_or_name with params:"
            f"id_or_name: {id_or_name}, module: {module.__name__}, id_key: {id_key}, error: {e.message}"
        )
        LOGGER.error(f"Encountered an error processing value for {entity_name}: {e.message}")
        sys.exit(1)

def get_id_from_id_or_name(id_or_name, module, id_key):
    """
    Checks if id_or_name is a UUID.  If it is, returns it.  If not, assumes it is a name of a
    record and attempts to retrieve the UUID for that record using module.find
    Parameters
    ----------
    id_or_name - a string containing either a UUID or a name of a record
    module - the rest module corresponding to the type of record; must define a find function
    id_key - the key for the id in the record that would be returned

    Returns
    -------
    If id_or_name is a valid UUID, returns it.  If not, attempts to retrieve a record with name
    matching id_or_name and return the UUID for that record. If unsuccessful, raises a
    RecordNotFoundError
    """
    # Check if this is a valid uuid
    try:
        uuid.UUID(id_or_name)
        # If it's successful, return the id
        return id_or_name
    except ValueError:
        # If it's not a valid UUID, let's assume it's a name and try to get the uuid for that name
        try:
            id = find_id_by_name(id_or_name, module, id_key)
            # If it worked, return the id
            return id
        except RecordNotFoundError:
            raise

def find_id_by_name(name, module, id_key):
    """
    Accepts the name of a record and the module corresponding to the type of record and attempts to
    retrieve a record using module.find with the name and return the uuid.  If the record is not
    found, raises a RecordNotFoundError

    Parameters
    ----------
    name - the name of the record we're searching for
    module - the rest module corresponding to the type of record; must define a find function
    id_key - the key for the id in the record that would be returned

    Returns
    -------
    If a record is found, returns the UUID for that record.  If not, raises a RecordNotFoundError
    """
    # Use module.find to try to get a record with that name
    # Note: we're limiting to 2 because names are unique, so if we get 2 or more records, we'll
    # consider that a failure
    record = module.find(name=name, limit=2)
    # Try to load it into a dict so we can parse it
    try:
        record_as_json = json.loads(record)
    except json.JSONDecodeError:
        # If that failed, raise an error
        raise RecordNotFoundError(record)
    # If it loaded as anything other than an array, that's an error because the expected output
    # of find is an array of results
    if not isinstance(record_as_json, list):
        raise RecordNotFoundError(record)
    # If it's an array but has anything other than exactly one element, raise an error
    if not len(record_as_json) == 1:
        raise RecordNotFoundError(
            f"Attempt to retrieve record by name produced unexpected result: {record}"
        )
    # Now try to get the id
    if id_key in record_as_json[0]:
        return record_as_json[0][id_key]
    else:
        # If we didn't find it, raise an error
        raise RecordNotFoundError(
            f"Attempt to retrieve {id_key} by name failed with record: {record}"
        )



class RecordNotFoundError(Exception):
    """
    Represents a failure to retrieve the UUID for a named record
    """

    # Constructor takes a message that corresponds to the error returned by the call to find
    def __init__(self, message):
        self.message = message
