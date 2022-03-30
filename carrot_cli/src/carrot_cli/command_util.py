import json
import logging
import sys

import click

from .config import manager as config

LOGGER = logging.getLogger(__name__)

def delete(id, yes, entity, entity_name):
    """
    Calls entity's delete function with id
    If yes is false, it first checks to see if the entity belongs to the user and prompts them to confirm the delete
    if it does not.  Uses entity_name in the prompt
    """
    # Unless user specifies --yes flag, check first to see if the record exists and prompt to user to confirm delete if
    # they are not the creator
    if not yes:
        # Try to find the record by id
        record = json.loads(entity.find_by_id(id))
        # If the returned record has a created_by field that does not match the user email, prompt the user to confirm
        # the delete
        user_email = config.load_var("email")
        if "created_by" in record and record["created_by"] != user_email:
            # If they decide not to delete, exit
            if not click.confirm(
                    f"{entity_name} with id {id} was created by {record['created_by']}. Are you sure you want to delete?"
            ):
                LOGGER.info("Okay, aborting delete operation")
                sys.exit(0)

    print(entity.delete(id))

def delete_map(entity1_id, entity2_id, yes, map_entity, entity1_name, entity2_name):
    """
    Calls map_entity's delete map function with entity1_id and entity2_id
    If yes is false, it first checks to see if the entity belongs to the user and prompts them to confirm the delete
    if it does not.  Uses entity1_name and entity2_name in the prompt
    """
    # Unless user specifies --yes flag, check first to see if the record exists and prompt to user to confirm delete if
    # they are not the creator
    if not yes:
        # Try to find the record by id
        record = json.loads(map_entity.find_map_by_ids(entity1_id, entity2_id))
        # If the returned record has a created_by field that does not match the user email, prompt the user to confirm
        # the delete
        user_email = config.load_var("email")
        if "created_by" in record and record["created_by"] != user_email:
            # If they decide not to delete, exit
            if not click.confirm(
                    f"Mapping for {entity1_name} with id {entity1_id} and {entity2_name} with id {entity2_id} was "
                    f"created by {record['created_by']}. Are you sure you want to delete?"
            ):
                LOGGER.info("Okay, aborting delete operation")
                sys.exit(0)

    print(map_entity.delete_map_by_ids(entity1_id, entity2_id))
