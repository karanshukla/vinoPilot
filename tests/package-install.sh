#!/usr/bin/env bash
# Installs a packed dist/'s .deb on Debian and its .rpm on Fedora, each in a
# throwaway container, the way a user would: the package manager resolves
# the dependencies, both commands run, the libraries the window opens at
# startup are there, and vinopilot sees itself as that distro's package.
#
#   tests/package-install.sh dist 0.1.0
set -euo pipefail
dist="$(cd "${1:?dist dir}" && pwd)" version="${2:?version}"

# Run inside the container, after the install: its shell expands these.
# shellcheck disable=SC2016
check='
set -eu
test "$(vinopilot --version)" = "vinopilot $VERSION"
test "$(vinopilot-gui --version)" = "vinopilot-gui $VERSION"
for lib in libEGL.so.1 libxkbcommon.so.0 libwayland-client.so.0 libX11.so.6; do
    ldconfig -p | grep -q "$lib" || { echo "no $lib after the install" >&2; exit 1; }
done
vinopilot list --json | tr -d " \n" | grep -q "\"system_package\":{\"kind\":\"$KIND\",\"packages\":\[\"vinopilot\"\]" ||
    { echo "vinopilot does not see its own $KIND package" >&2; exit 1; }
'

run() {
    local image="$1" kind="$2" install="$3"
    docker run --rm -v "$dist:/dist:ro" -e VERSION="$version" -e KIND="$kind" -e HOME=/tmp \
        "$image" sh -c "$install && $check"
    echo "ok - the .$kind installs and runs on $image ($version)"
}

run debian:stable-slim deb \
    'apt-get update -qq && apt-get install -y -qq --no-install-recommends /dist/vinopilot-x86_64.deb > /dev/null'
run fedora:latest rpm \
    'dnf install -y -q --setopt=install_weak_deps=False /dist/vinopilot-x86_64.rpm > /dev/null'
