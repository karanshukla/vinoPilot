# Security

`vinoPilot` runs under your regular user account and delegates to each project's own installer. Every action is a short shell script (inspectable with `vinopilot steps APP ACTION`) that calls the app's installer, `uv`, `cargo`, or `git`. Only `vinoAuthFace` requires elevated privileges; its privileged steps run through `pkexec` and prompt for your password each time.

Network requests happen only on demand: clicking **Check for updates** or running `vinopilot check` queries GitHub's releases API, PyPI, and crates.io, and runs `git fetch` in your local checkouts. Install and update actions download whatever the target app's own installer fetches.

## Reporting

Report vulnerabilities through [private vulnerability reporting](https://github.com/karanshukla/vinopilot/security/advisories/new). Do not open a public issue for anything exploitable.

## Threat model

| Threat | Mitigation |
|---|---|
| Tampered app release asset | Each app's installer verifies downloaded tarballs or wheels against the release's `SHA256SUMS` and aborts on a mismatch. `vinoPilot`'s own release does the same and includes signed build provenance (`gh attestation verify`). This catches corrupted or swapped assets, though it does not protect against a compromised upstream release. |
| Tampered installer script | Release installs fetch the app's `install.sh` over HTTPS from its latest release, matching the command in each app's README. Updates prefer the verified local copy saved during initial installation (`<app>-upgrade`). |
| Malicious version string or tag from a registry | Version strings and tags displayed in the UI or passed to installers are restricted to ASCII letters, digits, `.`, `-`, and `+`, with a 32-character maximum; anything else is rejected. |
| Paths containing shell metacharacters | Checkout paths and repository URLs are passed to scripts via environment variables rather than interpolated into shell source text. |
| Accidental deletion of local Git work | Uninstalling a source install or `vinoUserInterface` moves the checkout to the trash (`gio`, `kioclient`, or `trash-put`) only when the repo is clean, refusing if it contains uncommitted changes, unpushed commits, or stashed work. Uninstalling non-source installations never touches Git checkouts. |
| Unintended root execution beyond `vinoAuthFace` | Only catalog steps explicitly marked `root` invoke `pkexec`, and a unit test enforces that only `vinoAuthFace` steps are marked `root`. |
| Compromised dependency or GitHub Action | Crates are restricted to crates.io (`cargo deny`), `Cargo.lock` is enforced with `--locked`, every CI action is pinned to a commit SHA, Dependabot waits 7 days before opening PRs, and nothing auto-merges. |
