import mockito
import pytest

from carrot_cli import email_util
from carrot_cli.config import manager as config


@pytest.fixture(autouse=True)
def no_email():
    mockito.when(config).load_var_no_error("email").thenReturn(None)


@pytest.fixture(
    params=[
        {"email": "kevin@example.com", "result": True},
        {"email": "not_an_email_address@hello", "result": False},
        {"email": "not_an_email_address", "result": False},
        {"email": "not_an_email_address.hello", "result": False},
        {"email": "", "result": False},
    ]
)
def verify_email_data(request):
    return request.param


def test_verify_email(verify_email_data):
    assert (
        email_util.verify_email(verify_email_data["email"])
        == verify_email_data["result"]
    )
