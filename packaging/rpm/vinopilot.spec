# Wraps the release's prebuilt binaries: nothing is compiled here.
# .github/package-distro.sh passes `pkg_version` and `stage`, the tree to install.
Name:           vinopilot
Version:        %{pkg_version}
Release:        1
Summary:        Install, update, and uninstall the vinoPilot apps
License:        MIT
URL:            https://vinopilot.ca/
ExclusiveArch:  x86_64

# The same bytes as the tarball's: no stripping, no debuginfo, no build-id links.
%global debug_package %{nil}
%global __os_install_post %{nil}
%global _build_id_links none

Requires:       bash
Requires:       curl
Requires:       tar
# The window dlopens these, so rpm's own scan doesn't see them. By soname:
# Fedora and openSUSE name the packages differently.
Requires:       libEGL.so.1()(64bit)
Requires:       libxkbcommon.so.0()(64bit)
Requires:       libxkbcommon-x11.so.0()(64bit)
Requires:       libwayland-client.so.0()(64bit)
Requires:       libwayland-egl.so.1()(64bit)
Requires:       libX11.so.6()(64bit)
Requires:       libXcursor.so.1()(64bit)
Requires:       libXi.so.6()(64bit)
Recommends:     git
Recommends:     polkit

%description
Install, update, and uninstall the vinoPilot apps from one place: vinoWhisper,
vinoAuthFace, vinoOpenEffects, vinoPop, vinoSpeak, vinoSearch, vinoWatch, and
ovfetch. Each app ships its own checksum-verified installer; vinoPilot detects
how each app was installed on your system, checks for updates from the matching
source, and runs the appropriate installer.

This package includes the vinopilot CLI and the vinopilot-gui desktop app.
Managed apps still install into your home directory through their own
installers.

%install
cp -a %{stage}/. %{buildroot}/

%files
%license /usr/share/licenses/vinopilot/LICENSE
%doc /usr/share/doc/vinopilot/README.md
/usr/bin/vinopilot
/usr/bin/vinopilot-gui
/usr/share/applications/io.github.karanshukla.vinopilot.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.karanshukla.vinopilot.svg
/usr/share/metainfo/io.github.karanshukla.vinopilot.metainfo.xml
