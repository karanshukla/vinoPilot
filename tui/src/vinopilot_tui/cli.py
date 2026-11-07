"""The Rust `vinopilot` command, driven through `--json` (docs/json-api.md).

Nothing here knows the catalog: which apps exist, how they install, what's
newer. That all comes back as JSON from the command.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from collections.abc import Iterator
from dataclasses import dataclass, field

INSTALLER = "https://github.com/karanshukla/vinopilot/releases/latest/download/install.sh"

# How the missing-command screen gets the command: the release installer,
# hash-checked by itself, without the window. Patched in tests.
INSTALL_COMMAND = ["bash", "-c", f"curl -fsSL {INSTALLER} | bash -s -- --no-gui"]


def find() -> str | None:
    """`$VINOPILOT_BIN`, else `vinopilot` on PATH or in `~/.local/bin`
    (where the release installer puts it, PATH or not)."""
    pinned = os.environ.get("VINOPILOT_BIN")
    if pinned:
        return pinned if os.access(pinned, os.X_OK) else None
    path = os.environ.get("PATH", os.defpath)
    local = os.path.join(os.path.expanduser("~"), ".local", "bin")
    return shutil.which("vinopilot", path=f"{path}{os.pathsep}{local}")


class CommandError(Exception):
    """The command exited non-zero where a document was expected."""


@dataclass(frozen=True)
class Event:
    """One line of an action's output, or its end."""

    kind: str  # "line" or "done"
    text: str = ""
    app: str = ""
    channel: str = ""
    outcome: str = ""  # done, stopped, failed
    reason: str = ""

    @staticmethod
    def parse(line: str) -> Event:
        try:
            doc = json.loads(line)
        except ValueError:
            # The command's own stderr, merged in: usage errors, `✗ ...`.
            return Event("line", text=line)
        if not isinstance(doc, dict) or doc.get("event") not in ("line", "done"):
            return Event("line", text=line)
        if doc["event"] == "line":
            return Event("line", text=str(doc.get("text", "")))
        result = doc.get("result") or {}
        return Event(
            "done",
            app=str(doc.get("app", "")),
            channel=str(doc.get("channel", "")),
            outcome=str(result.get("outcome", "failed")),
            reason=str(result.get("reason") or ""),
        )


@dataclass
class Job:
    """A running install, update or uninstall; iterate `events()` to follow it."""

    process: subprocess.Popen[str]
    label: str
    done: list[Event] = field(default_factory=list)

    def events(self) -> Iterator[Event]:
        assert self.process.stdout is not None
        last = ""
        for raw in self.process.stdout:
            line = raw.rstrip("\n")
            event = Event.parse(line)
            if event.kind == "done":
                self.done.append(event)
            else:
                last = event.text or last
            yield event
        code = self.process.wait()
        if not self.done:
            # Exited without saying so: a usage error, a crash, or stopped
            # before it started.
            outcome = "stopped" if code < 0 else ("done" if code == 0 else "failed")
            event = Event("done", outcome=outcome, reason=last if outcome == "failed" else "")
            self.done.append(event)
            yield event

    def stop(self) -> None:
        """Ends the job. A pkexec child is root's and outlives this."""
        if self.process.poll() is None:
            self.process.terminate()


@dataclass(frozen=True)
class Pilot:
    """A `vinopilot` binary."""

    bin: str

    def _args(self, *args: str, source_root: str | None = None) -> list[str]:
        argv = [self.bin]
        if source_root:
            argv += ["--source-root", source_root]
        return argv + list(args)

    def _run(self, *args: str, json_out: bool = True) -> str:
        argv = self._args(*args)
        if json_out:
            argv.insert(1, "--json")
        out = subprocess.run(argv, capture_output=True, text=True, stdin=subprocess.DEVNULL)
        if out.returncode != 0:
            raise CommandError((out.stderr or out.stdout).strip() or f"exit {out.returncode}")
        return out.stdout

    def version(self) -> str:
        """`0.1.0`, from `vinopilot --version`."""
        return self._run("--version", json_out=False).split()[-1]

    def list(self) -> dict:
        """The offline document: what's installed, and how."""
        return json.loads(self._run("list"))

    def check(self) -> dict:
        """The same, plus `latest` and a judged `update`. Goes online."""
        return json.loads(self._run("check"))

    def steps(self, app: str, action: str, via: str | None = None) -> str:
        """The shell an action would run, as text."""
        args = ["steps", app, action]
        if via:
            args += ["--via", via]
        return self._run(*args, json_out=False)

    def source_root(self) -> str:
        return json.loads(self._run("config"))["source_root"]

    def set_source_root(self, path: str) -> None:
        self._run("config", "source_root", path, json_out=False)

    def start(self, label: str, *args: str) -> Job:
        """Starts an action; its output arrives as `Event`s, stderr merged in."""
        argv = self._args(*args)
        argv.insert(1, "--json")
        process = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        return Job(process, label)

    def install(self, app: str, via: str) -> Job:
        return self.start(f"Installing {app}", "install", app, "--via", via)

    def update(self, app: str) -> Job:
        return self.start(f"Updating {app}", "update", app)

    def update_all(self) -> Job:
        return self.start("Updating everything", "update", "--all")

    def uninstall(self, app: str) -> Job:
        return self.start(f"Uninstalling {app}", "uninstall", app, "--yes")


def installer() -> Job:
    """Runs the release installer, to get the command when there is none."""
    process = subprocess.Popen(
        INSTALL_COMMAND,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    return Job(process, "Installing vinopilot")
