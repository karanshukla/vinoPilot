from pathlib import Path

import pytest
from textual.widgets import DataTable, Input, RichLog

from vinopilot_tui import cli
from vinopilot_tui.app import ChooseScreen, ConfirmScreen, MissingScreen, PilotApp, TextScreen


async def settle(app: PilotApp, pilot) -> None:
    """Lets the thread workers finish and their results land."""
    await app.workers.wait_for_complete()
    await pilot.pause()
    # A job's end queues a rescan from the worker thread.
    await app.workers.wait_for_complete()
    await pilot.pause()


def cell(app: PilotApp, row: int, column: int) -> str:
    return str(app.query_one(DataTable).get_cell_at((row, column)))


def log_text(app: PilotApp) -> str:
    return "\n".join(str(line.text) for line in app.query_one("#log", RichLog).lines)


async def test_lists_components_offline(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        table = app.query_one(DataTable)
        assert table.row_count == 3
        assert cell(app, 0, 0) == "vinoWhisper"
        assert cell(app, 0, 1) == "1.3.0"
        assert cell(app, 0, 2) == "Source"
        assert cell(app, 1, 2) == "not installed"
        assert cell(app, 2, 2) == "crates.io"
        assert cell(app, 0, 3) == ""
        assert cell(app, 0, 4) == "Live captions and voice typing"
        assert cell(app, 2, 4) == (
            "Two copies: /home/someone/.cargo/bin/ovfetch runs, "
            "/home/someone/.local/bin/ovfetch doesn't"
        )
    assert ["--json", "check"] not in calls()


async def test_check_fills_the_update_column(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("c")
        await settle(app, pilot)
        assert cell(app, 0, 3) == "3 commits available"
        assert cell(app, 1, 3) == "latest v2.0.0"
        assert cell(app, 2, 3) == "up to date"
        assert "1 update available" in app.status_text
    assert ["--json", "check"] in calls()


async def test_install_asks_which_way_then_runs(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        app.query_one(DataTable).move_cursor(row=1)
        await pilot.press("i")
        await pilot.pause()
        assert isinstance(app.screen, ChooseScreen)
        await pilot.press("down", "enter")
        await settle(app, pilot)
        text = log_text(app)
        assert "== Installing vinowatch" in text
        assert "checksum verified" in text
        assert "✓ Done vinowatch" in text
    assert ["--json", "install", "vinowatch", "--via", "source"] in calls()
    # The table is rescanned afterwards, offline since nothing was checked.
    assert calls()[-1] == ["--json", "list"]


async def test_install_of_something_installed_is_refused(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("i")
        await settle(app, pilot)
        assert isinstance(app.screen, type(app.screen)) and not isinstance(app.screen, ChooseScreen)
    assert not any(c[1:2] == ["install"] for c in calls())


async def test_uninstall_needs_a_yes(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("x")
        await pilot.pause()
        assert isinstance(app.screen, ConfirmScreen)
        await pilot.press("n")
        await settle(app, pilot)
        assert not any("uninstall" in c for c in calls())
        await pilot.press("x")
        await pilot.pause()
        await pilot.press("y")
        await settle(app, pilot)
        assert "✓ Done vinowhisper" in log_text(app)
    assert ["--json", "uninstall", "vinowhisper", "--yes"] in calls()


async def test_a_failure_shows_the_reason(fake_bin: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        app.run_job(app.pilot.install("vinofail", "release"))
        await settle(app, pilot)
        assert "✗ Failed vinofail: the installer said no" in log_text(app)


async def test_update_all_rechecks_afterwards(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("c")
        await settle(app, pilot)
        await pilot.press("a")
        await settle(app, pilot)
    assert ["--json", "update", "--all"] in calls()
    assert calls()[-1] == ["--json", "check"]


async def test_steps_are_shown_not_run(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("s")
        await pilot.pause()
        assert isinstance(app.screen, ChooseScreen)
        await pilot.press("enter")
        await pilot.pause()
        assert isinstance(app.screen, TextScreen)
        assert "release_run" in app.screen.text
        await pilot.press("escape")
        await pilot.pause()
    assert ["steps", "vinowhisper", "install"] in calls()
    assert not any(c[1:2] == ["install"] for c in calls())


async def test_source_root_is_set_through_the_command(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("r")
        await pilot.pause()
        app.screen.query_one(Input).value = "/tmp/code"
        await pilot.press("enter")
        await settle(app, pilot)
    assert ["config", "source_root", "/tmp/code"] in calls()


async def test_missing_command_offers_the_installer(
    no_bin: Path, monkeypatch: pytest.MonkeyPatch, calls
) -> None:
    from conftest import write_fake

    staged = write_fake(no_bin.parent / "staged")
    monkeypatch.setattr(
        cli,
        "INSTALL_COMMAND",
        ["sh", "-c", f'echo "== pretend download"; cp "{staged}" "{no_bin}"'],
    )
    app = PilotApp()
    async with app.run_test() as pilot:
        await pilot.pause()
        assert isinstance(app.screen, MissingScreen)
        await pilot.click("#install")
        await settle(app, pilot)
        assert not isinstance(app.screen, MissingScreen)
        assert app.query_one(DataTable).row_count == 3
        assert "✓ Installed the vinopilot command" in log_text(app)
    assert ["--json", "list"] in calls()


async def test_missing_command_quit(no_bin: Path) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await pilot.pause()
        await pilot.click("#quit")
        await pilot.pause()
    assert app.return_code == 0 or app._exit


async def test_vinopilot_is_in_the_status_line_not_the_table(fake_bin: Path) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        assert "vinoPilot" not in [cell(app, row, 0) for row in range(3)]
        assert app.status_text.startswith("vinoPilot 9.9.9")


async def test_p_checks_then_updates_then_restarts(fake_bin: Path, calls) -> None:
    app = PilotApp()
    async with app.run_test() as pilot:
        await settle(app, pilot)
        await pilot.press("p")
        await settle(app, pilot)
        assert ["--json", "check"] in calls()
        assert app.status_text.startswith("vinoPilot 9.9.9, p updates to v10.0.0")
        # vinoPilot's own update isn't one of the table's.
        assert "1 update available" in app.status_text

        await pilot.press("p")
        await settle(app, pilot)
        assert ["--json", "update", "vinopilot"] in calls()
        assert app.status_text.startswith("vinoPilot 10.0.0 installed, p restarts into it")

        await pilot.press("p")
        await pilot.pause()
    assert app.restart
