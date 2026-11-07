# Changelog

All notable changes to vinoPilot are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-06

### Features

- Install, update and uninstall vinoWhisper, vinoAuthFace, vinoOpenEffects, vinoPop, vinoSpeak, vinoSearch, vinoWatch, ovfetch, vinoUserInterface and vinoPilot itself, from their GitHub releases, a source checkout, PyPI or crates.io
- Detect how each is already installed, and check GitHub, PyPI, crates.io and checkouts for something newer
- `vinopilot` command with `--json` output as the API for other frontends
- `vinopilot-gui` window: a card grid with Update all, and the installer's output underneath
- `install.sh`: hash-checked release install, source install from a checkout, `--no-gui`, `--uninstall`
