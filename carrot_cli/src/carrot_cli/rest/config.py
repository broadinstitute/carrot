from ..config import manager as config_manager
from . import request_handler


def get_cromwell_address():
    """Sends a request to carrot to retrieve the address of the cromwell server it uses"""
    server_address = config_manager.load_var("carrot_server_address")
    return request_handler.send_request(
        "GET",
        f"http://{server_address}/api/v1/config/cromwell",
        expected_format=request_handler.ResponseFormat.TEXT,
    )
