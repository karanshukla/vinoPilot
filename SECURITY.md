# Security

vinoPilot runs as you and runs other projects' installers. It installs nothing itself: every action is a short shell script (`vinopilot steps APP ACTION` prints it) that calls the app's own installer, `uv`, `cargo` or `git`. vinoAuthFace's actions run as root through `pkexec`, which asks for your password each time. Nothing else asks for root.

It goes online only when you ask: **Check for updates** / `vinopilot check` reads GitHub's releases API, PyPI's and crates.io's JSON, and runs `git fetch` in your checkouts. Installs and updates download what the app's installer downloads.

## Reporting

Use [private vulnerability reporting](https://github.com/karanshukla/vinopilot/security/advisories/new). Do not open a public issue for anything exploitable.

## Threat model

| Threat | Defence |
|---|---|
| A tampered app release | Each app's installer checks its tarball or wheel against the release's `SHA256SUMS` and refuses a mismatch. vinoPilot's own release does the same for itself, and the release is signed with build provenance (`gh attestation verify`). This catches a swapped or corrupted asset, not a compromised release. |
| A tampered installer script | Release installs fetch the app's `install.sh` over HTTPS from its latest release and run it, exactly like the one-liner each README gives. Updates prefer the checked copy the first install kept (`<app>-upgrade`). |
| A hostile version string or tag from a registry | Tags and versions shown in the window or passed to an installer are held to letters, digits, `.`, `-`, `+` and 32 characters; anything else is dropped. |
| Paths with shell metacharacters | The source folder and repo URLs reach scripts as environment variables, never formatted into the script text. |
| Deleting your work | Deleting the vinoUserInterface checkout refuses when it has uncommitted changes or unpushed commits. Uninstalling an app never deletes its checkout. |
| Root beyond vinoAuthFace | Only steps marked `root` in the catalog use `pkexec`; a test asserts that only vinoAuthFace's are. |
| Compromised dependency or action | Crates from crates.io only (`cargo deny`), `Cargo.lock` enforced with `--locked`, every CI action pinned to a commit, Dependabot waits 7 days and nothing auto-merges. |
