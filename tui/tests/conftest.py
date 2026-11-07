import json
import os
import stat
import sys
from pathlib import Path

import pytest

FAKE = Path(__file__).with_name("fake_vinopilot.py")


def write_fake(path: Path) -> Path:
    """An executable `vinopilot` that runs fake_vinopilot.py with this Python."""
    path.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{FAKE}" "$@"\n')
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


@pytest.fixture
def fake_bin(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """The fake on `$VINOPILOT_BIN`, logging its calls to `calls` (see `calls`)."""
    binary = write_fake(tmp_path / "vinopilot")
    monkeypatch.setenv("VINOPILOT_BIN", str(binary))
    monkeypatch.setenv("FAKE_VINOPILOT_LOG", str(tmp_path / "calls.log"))
    return binary


@pytest.fixture
def calls(tmp_path: Path):
    """Every argv the fake was run with, so far."""

    def read() -> list[list[str]]:
        log = tmp_path / "calls.log"
        if not log.exists():
            return []
        return [json.loads(line) for line in log.read_text().splitlines()]

    return read


@pytest.fixture
def no_bin(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """`$VINOPILOT_BIN` pointing where nothing is, yet."""
    binary = tmp_path / "vinopilot"
    monkeypatch.setenv("VINOPILOT_BIN", str(binary))
    monkeypatch.setenv("FAKE_VINOPILOT_LOG", str(tmp_path / "calls.log"))
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("PATH", os.defpath)
    return binary
