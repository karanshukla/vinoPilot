"""Every `test_file.py::test_name` a docstring or comment in src/ cites after
"pinned by" must be a test in tests/, so a rule that names its test fails when
the test is renamed or deleted. See docs/comment-style.md. Runs under pytest,
or alone: `python3 tui/tests/test_doc_links.py`."""

from __future__ import annotations

import re
from pathlib import Path

TUI = Path(__file__).resolve().parent.parent
CITED = re.compile(r"`(test_\w+\.py)::(\w+)`")


def cited_tests(text: str) -> list[tuple[int, str, str]]:
    """(line, file, test) for each citation; one may wrap onto the next line."""
    pinned = text.find("pinned by")
    found = []
    while pinned != -1:
        end = text.find(")", pinned)
        span = text[pinned : end if end != -1 else len(text)]
        line = text.count("\n", 0, pinned) + 1
        found += [(line, f, name) for f, name in CITED.findall(span)]
        pinned = text.find("pinned by", pinned + 1)
    return found


def resolves(tests: Path, file: str, name: str) -> bool:
    path = tests / file
    if not name.startswith("test_") or not path.is_file():
        return False
    return re.search(rf"^(async )?def {name}\(", path.read_text(), re.MULTILINE) is not None


def broken_citations(src: Path, tests: Path) -> list[str]:
    broken = []
    for path in sorted(src.rglob("*.py")):
        for line, file, name in cited_tests(path.read_text()):
            if not resolves(tests, file, name):
                broken.append(f"{path.relative_to(src.parent)}:{line}: `{file}::{name}`")
    return broken


def test_every_cited_test_exists() -> None:
    broken = broken_citations(TUI / "src", TUI / "tests")
    assert not broken, "no such test:\n" + "\n".join(broken)


def test_the_check_finds_a_cited_test_and_rejects_a_missing_one(tmp_path: Path) -> None:
    (tmp_path / "test_x.py").write_text("def test_real():\n    pass\n\ndef helper():\n    pass\n")
    text = '"""(pinned by `test_x.py::test_real`,\n`test_x.py::helper`, `test_x.py::test_gone`)"""'
    cited = cited_tests(text)
    assert cited == [
        (1, "test_x.py", "test_real"),
        (1, "test_x.py", "helper"),
        (1, "test_x.py", "test_gone"),
    ]
    assert resolves(tmp_path, "test_x.py", "test_real")
    assert not resolves(tmp_path, "test_x.py", "helper")
    assert not resolves(tmp_path, "test_x.py", "test_gone")
    assert not resolves(tmp_path, "test_y.py", "test_real")


if __name__ == "__main__":
    import sys

    broken = broken_citations(TUI / "src", TUI / "tests")
    print("no such test:\n" + "\n".join(broken) if broken else "every cited test exists")
    sys.exit(1 if broken else 0)
