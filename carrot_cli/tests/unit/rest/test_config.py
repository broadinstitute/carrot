import mockito

from carrot_cli.config import manager as config
from carrot_cli.rest import config as config_rest
from carrot_cli.rest import request_handler


def test_get_cromwell_address():
    # Set all requests to return None so only the one we expect will return a value
    mockito.when(request_handler).send_request(...).thenReturn(None)
    # Mock up request response
    address = "http://example.com/api/v1/config/cromwell"
    mockito.when(request_handler).send_request(
        "GET", address, expected_format=request_handler.ResponseFormat.TEXT
    ).thenReturn("example.com/cromwell")
    # Instead of setting and loading config from a file, we'll just mock this
    mockito.when(config).load_var("carrot_server_address").thenReturn("example.com")
    # Run test
    result = config_rest.get_cromwell_address()
    assert result == "example.com/cromwell"
