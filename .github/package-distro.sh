#!/bin/sh
# Wraps the release's binaries as a .deb and an .rpm:
#
#   .github/package-distro.sh SRC OUT
#
# SRC is the unpacked tarball (vinopilot, vinopilot-gui, LICENSE, README.md);
# OUT gets vinopilot-x86_64.deb and vinopilot-x86_64.rpm. Nothing is compiled
# or stripped, so both carry the same bytes as the tarball. Needs dpkg-deb
# and rpmbuild (`apt-get install rpm` on Ubuntu).
set -eu

src="$(cd "$1" && pwd)"
mkdir -p "$2"
out="$(cd "$2" && pwd)"
here="$(cd "$(dirname "$0")/../packaging" && pwd)"
id=io.github.karanshukla.vinopilot
version="$("$src/vinopilot" --version | cut -d' ' -f2)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# What both install, under /usr.
stage() {
    install -Dm755 -t "$1/usr/bin" "$src/vinopilot" "$src/vinopilot-gui"
    install -Dm644 -t "$1/usr/share/metainfo" "$here/$id.metainfo.xml"
    install -Dm644 -t "$1/usr/share/doc/vinopilot" "$src/README.md"
    # The binary writes its own menu entry and icon, as for every other
    # install; only Exec differs, which would be this build's path.
    XDG_DATA_HOME="$1/usr/share" "$src/vinopilot-gui" --install-desktop > /dev/null
    sed -i 's|^Exec=.*|Exec=vinopilot-gui|' "$1/usr/share/applications/$id.desktop"
    rm -f "$1/usr/share/applications/mimeinfo.cache"
}

deb="$work/deb"
stage "$deb"
install -Dm644 "$src/LICENSE" "$deb/usr/share/doc/vinopilot/copyright"
mkdir "$deb/DEBIAN"
sed -e "s/@VERSION@/$version/" -e "s/@SIZE@/$(du -sk "$deb/usr" | cut -f1)/" \
    "$here/deb/control" > "$deb/DEBIAN/control"
# xz: Debian's dpkg can't read the zstd that Ubuntu's writes by default.
dpkg-deb --root-owner-group -Zxz --build "$deb" "$out/vinopilot-x86_64.deb" > /dev/null

rpm="$work/rpm"
stage "$rpm"
install -Dm644 -t "$rpm/usr/share/licenses/vinopilot" "$src/LICENSE"
rpmbuild -bb --quiet --target x86_64 \
    --define "_topdir $work/rpmbuild" \
    --define "_rpmdir $work" \
    --define "_rpmfilename vinopilot.rpm" \
    --define "_binary_payload w9.xzdio" \
    --define "pkg_version $version" \
    --define "stage $rpm" \
    "$here/rpm/vinopilot.spec"
mv "$work/vinopilot.rpm" "$out/vinopilot-x86_64.rpm"
