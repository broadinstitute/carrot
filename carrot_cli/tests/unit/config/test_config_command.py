from click.testing import CliRunner

import mockito
import pytest
from carrot_cli.__main__ import main_entry as carrot
from carrot_cli.config import manager as config
from carrot_cli.rest import config as config_rest

@pytest.fixture(autouse=True)
def unstub():
    yield
    mockito.unstub()

def test_cromwell():
    mockito.when(config_rest).get_cromwell_address().thenReturn("example.com/cromwell")
    runner = CliRunner()
    result = runner.invoke(carrot, ["config", "cromwell"])
    assert result.output == "example.com/cromwell" + "\n"