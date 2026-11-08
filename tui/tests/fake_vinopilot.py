"""A `vinopilot` that answers with canned JSON and logs what it was asked.

Just enough of docs/json-api.md for the tests: three components, one per
state, and actions that print line events then done.
"""

import json
import os
import sys

DOC = {
    "vinopilot": "9.9.9",
    "source_root": "/home/someone/src",
    "components": [
        {
            "id": "vinowhisper",
            "name": "vinoWhisper",
            "blurb": "Live captions and voice typing",
            "repo": "karanshukla/vinoWhisper",
            "channels": ["release", "source", "package"],
            "registry": "pypi",
            "needs_root": False,
            "installed": {"channel": "source", "version": "1.3.0"},
            "checkout": {"path": "/home/someone/src/vinoWhisper", "describe": "v1.3.0-3-gabc"},
            "shadowed": [],
            "update": {"state": "unknown", "reason": None},
        },
        {
            "id": "vinowatch",
            "name": "vinoWatch",
            "blurb": "Watches the NPU",
            "repo": "karanshukla/vinoWatch",
            "channels": ["release", "source"],
            "registry": None,
            "needs_root": False,
            "installed": None,
            "checkout": None,
            "shadowed": [],
            "update": {"state": "unknown", "reason": None},
        },
        {
            "id": "ovfetch",
            "name": "ovfetch",
            "blurb": "Fetches the OpenVINO runtime an NPU needs",
            "repo": "karanshukla/ovfetch",
            "channels": ["release", "source", "package"],
            "registry": "crates.io",
            "needs_root": False,
            "installed": {"channel": "package", "version": "1.0.0"},
            "checkout": None,
            "shadowed": [
                {
                    "command": "ovfetch",
                    "runs": "/home/someone/.cargo/bin/ovfetch",
                    "hidden": "/home/someone/.local/bin/ovfetch",
                }
            ],
            "update": {"state": "unknown", "reason": None},
        },
    ],
}

CHECKED = {
    "vinowhisper": (
        {"release": {"value": "v1.3.1"}, "package": {"value": "1.3.1"}, "behind": {"value": 3}},
        {"state": "available", "to": "3 commits"},
    ),
    "vinowatch": (
        {"release": {"value": "v2.0.0"}, "package": None, "behind": None},
        {"state": "unknown", "reason": None},
    ),
    "ovfetch": (
        {"release": {"value": "v1.0.0"}, "package": {"value": "1.0.0"}, "behind": None},
        {"state": "up_to_date"},
    ),
}


def main() -> int:
    args = sys.argv[1:]
    log = os.environ.get("FAKE_VINOPILOT_LOG")
    if log:
        with open(log, "a") as f:
            f.write(json.dumps(args) + "\n")
    json_out = "--json" in args
    args = [a for a in args if a != "--json"]
    if args == ["--version"]:
        print("vinopilot 9.9.9")
        return 0
    command = args[0] if args else "help"
    if command in ("list", "check"):
        doc = json.loads(json.dumps(DOC))
        if command == "check":
            for c in doc["components"]:
                c["latest"], c["update"] = CHECKED[c["id"]]
        print(json.dumps(doc, indent=2) if json_out else "APP ...")
        return 0
    if command == "config":
        if len(args) == 3:
            print(f"source_root = {args[2]}")
        else:
            print(json.dumps({"source_root": DOC["source_root"]}))
        return 0
    if command == "steps":
        print(f"# Installing {args[1]} via Release\nset -euo pipefail\nrelease_run install.sh")
        return 0
    if command in ("install", "update", "uninstall"):
        app = args[1] if len(args) > 1 and not args[1].startswith("--") else "vinowatch"
        channel = args[args.index("--via") + 1] if "--via" in args else "release"
        if command == "uninstall" and "--yes" not in args:
            print("vinopilot: pass --yes to uninstall without a terminal", file=sys.stderr)
            return 2
        for text in ("== Downloading", "checksum verified"):
            print(json.dumps({"event": "line", "text": text}), flush=True)
        if app == "vinofail":
            result = {"outcome": "failed", "reason": "the installer said no"}
        else:
            result = {"outcome": "done"}
        print(json.dumps({"event": "done", "app": app, "channel": channel, "result": result}))
        return 0 if result["outcome"] == "done" else 1
    print(f"vinopilot: unknown command {command} (see --help)", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
