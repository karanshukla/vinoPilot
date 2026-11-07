from pathlib import Path

import pytest

from vinopilot_tui import cli
from vinopilot_tui.cli import CommandError, Event, Pilot


def test_find_prefers_the_env_var(fake_bin: Path) -> None:
    assert cli.find() == str(fake_bin)


def test_find_is_none_when_the_env_var_points_nowhere(no_bin: Path) -> None:
    assert cli.find() is None


def test_find_falls_back_to_local_bin(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    from conftest import write_fake

    (tmp_path / ".local" / "bin").mkdir(parents=True)
    binary = write_fake(tmp_path / ".local" / "bin" / "vinopilot")
    monkeypatch.delenv("VINOPILOT_BIN", raising=False)
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("PATH", str(tmp_path / "empty"))
    assert cli.find() == str(binary)


def test_documents(fake_bin: Path, calls) -> None:
    pilot = Pilot(str(fake_bin))
    assert pilot.version() == "9.9.9"
    doc = pilot.list()
    assert [c["id"] for c in doc["components"]] == ["vinowhisper", "vinowatch", "ovfetch"]
    assert "latest" not in doc["components"][0]
    checked = pilot.check()
    assert checked["components"][0]["update"] == {"state": "available", "to": "3 commits"}
    assert pilot.source_root() == "/home/someone/src"
    assert "release_run" in pilot.steps("vinowatch", "install")
    assert [c[:2] for c in calls()] == [
        ["--version"],
        ["--json", "list"],
        ["--json", "check"],
        ["--json", "config"],
        ["steps", "vinowatch"],
    ]


def test_a_usage_error_raises(fake_bin: Path) -> None:
    with pytest.raises(CommandError, match="unknown command"):
        Pilot(str(fake_bin))._run("bogus")


def test_install_streams_lines_then_done(fake_bin: Path, calls) -> None:
    job = Pilot(str(fake_bin)).install("vinowatch", "source")
    events = list(job.events())
    assert [e.kind for e in events] == ["line", "line", "done"]
    assert events[0].text == "== Downloading"
    assert events[-1] == Event("done", app="vinowatch", channel="source", outcome="done")
    assert calls()[-1] == ["--json", "install", "vinowatch", "--via", "source"]


def test_a_failed_install_says_why(fake_bin: Path) -> None:
    done = list(Pilot(str(fake_bin)).install("vinofail", "release").events())[-1]
    assert done.outcome == "failed"
    assert done.reason == "the installer said no"


def test_uninstall_passes_yes(fake_bin: Path, calls) -> None:
    done = list(Pilot(str(fake_bin)).uninstall("ovfetch").events())[-1]
    assert done.outcome == "done"
    assert calls()[-1] == ["--json", "uninstall", "ovfetch", "--yes"]


def test_update_all(fake_bin: Path, calls) -> None:
    list(Pilot(str(fake_bin)).update_all().events())
    assert calls()[-1] == ["--json", "update", "--all"]


def test_exiting_without_a_done_event_is_a_failure_with_the_last_line(fake_bin: Path) -> None:
    job = Pilot(str(fake_bin)).start("bogus", "bogus")
    events = list(job.events())
    assert events[-1].kind == "done"
    assert events[-1].outcome == "failed"
    assert "unknown command" in events[-1].reason


def test_stop_ends_the_job(fake_bin: Path, tmp_path: Path) -> None:
    import subprocess

    job = cli.Job(
        subprocess.Popen(["sleep", "30"], stdout=subprocess.PIPE, text=True),
        "sleeping",
    )
    job.stop()
    events = list(job.events())
    assert events == [Event("done", outcome="stopped")]


def test_parse_tolerates_plain_text() -> None:
    assert Event.parse("✗ something from stderr") == Event("line", text="✗ something from stderr")
    assert Event.parse('{"event":"other"}') == Event("line", text='{"event":"other"}')
