#!/bin/sh
# Builds the binaries and packs dist/ the way a release ships it: one
# tarball plus install.sh and SHA256SUMS, which install.sh checks downloads
# against. release.yml publishes the result; ci.yml builds and smoke-tests
# the same thing on every PR.
#
# vinopilot (the CLI) is static musl and runs anywhere. vinopilot-gui is
# glibc: eframe dlopens libEGL, libwayland and libX11, which a static musl
# binary can't. Build it on the oldest runner there is (ubuntu-22.04,
# glibc 2.35) so it runs on older distros too.
set -eu

cargo build --release --locked --target x86_64-unknown-linux-musl -p vinopilot
cargo build --release --locked --target x86_64-unknown-linux-gnu -p vinopilot-gui

rm -rf dist && mkdir -p dist/vinopilot
cp target/x86_64-unknown-linux-musl/release/vinopilot \
    target/x86_64-unknown-linux-gnu/release/vinopilot-gui \
    install.sh LICENSE README.md dist/vinopilot/
tar -czf dist/vinopilot-x86_64-linux.tar.gz -C dist vinopilot
rm -rf dist/vinopilot
# On its own too, for releases/latest/download/install.sh.
cp install.sh dist/
(cd dist && sha256sum vinopilot-x86_64-linux.tar.gz install.sh > SHA256SUMS)
