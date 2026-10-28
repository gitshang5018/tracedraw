# Updates

The browser build is always the latest: there Help > Updates opens the
releases page. Every other build checks.

## Where the releases are

The project publishes from `gitshang5018/tracedraw`. Two kinds of release
live there, and both are followed:

- the development builds, as the pre-release tagged `nightly`, whose files
  are replaced on every push to main and named after the commit
  (`tracedraw-0.19.9-dev.4c864d1-android-arm64-v8a.apk`);
- a version, as a release of its own (`v0.19.9`), named without the commit.

The check reads the list of releases
(`api.github.com/repos/gitshang5018/tracedraw/releases`), newest first, and
takes the first one that carries a file for this system and is newer than
the build that is running. The list rather than `/releases/latest`: that
address never names a pre-release, and the development builds are
pre-releases. A release with nothing for this system is passed over, so a
Linux build looks past the nightlies (APKs only) to the last release that
had an AppImage.

The list is read from GitHub directly, and so is the release page. A
release's file is downloaded through a proxy, for networks where GitHub's
file host is slow or out of reach: `https://github.adc.dpdns.org/` and then
the whole file address, as the proxy documents it
(`https://github.adc.dpdns.org/https://github.com/...`). Only a file of a
release (`/releases/download/`, or a host GitHub redirects to) is rewritten;
anything else is left as it is (`updates::through_proxy`).

## Which build is this

`updates::version()`: the version the packaging scripts passed in
(`TRACEDRAW_BUILD_VERSION`, through `build.rs`), else the crate version. A
version release reports `0.19.9`; a development build reports
`0.19.9-dev.4c864d1`, so two builds of the same version are told apart by
the commit they were made from. The Android workflow passes the same
version it puts in the APK's name.

Numbers compare as major, minor, patch. When they are equal:

- a development build is newer than the release of that version (the
  builds on main come after the tag);
- two development builds differ by their commit, and any difference counts
  as newer (the order of the release list is what says which came last);
- a release is never newer than a development build of the same version.

A development release carries the fixed tag "nightly", so its version is
read from its file names (`updates::release_version`).

## Which file

Download opens the link of the release file named exactly for this system,
its processor (x86_64, or arm64 for aarch64) and how this copy was
installed (`updates::asset_names`); the system's installer does the
installing:

- Linux run as an AppImage (`APPIMAGE` set): `tracedraw-<v>-linux-<arch>.AppImage`,
  where x86_64 looks for `linux-amd64` first and then `linux-x86_64`
  (releases up to 0.19.8). From 0.19.9 on the release names the x86_64
  AppImage `linux-amd64`, which sorts before `linux-arm64`: updaters of
  0.19.8 and earlier take the first AppImage in GitHub's list, sorted by
  name, and so get the x86_64 file (#1038). The tar.gz keeps
  `linux-x86_64`;
- Linux installed from the `.deb` (dpkg's list for `tracedraw` names the
  running program): `tracedraw_<v>_<amd64|arm64>.deb`;
- Linux unpacked from the tar.gz (a program outside `/usr`):
  `tracedraw-<v>-linux-<arch>.tar.gz`;
- Windows installed with setup: `tracedraw-<v>-windows-<arch>-setup.exe`;
  the portable copy (the `tracedraw-settings` folder next to the program):
  `tracedraw-<v>-windows-<arch>-portable.zip`;
- macOS: `tracedraw-<v>-macos-<arch>.dmg`;
- Android: `tracedraw-<v>-android-<abi>.apk`, one per ABI
  (`arm64-v8a`, `armeabi-v7a`, `x86_64`), and only the one this build was
  packed for.

When the release has no such file (an rpm or other system package under
`/usr`, a Flatpak or Snap, a processor without a build such as Windows on
arm64) no file is downloaded and Download opens the release page instead; a
file for another processor or package is never offered (#1038: the x86_64
AppImage took the arm64 one, listed first).

## Install in place

When the release's file for this system carries a SHA-256 digest (GitHub
gives one for every uploaded file) and there is somewhere to put it, the
dialog offers Install first:

- **A phone**: the APK is downloaded into the app's cache, its size and
  digest are checked, and the file is written into a `PackageInstaller`
  session and committed, so the system's own prompt asks the user to update
  TraceDraw. Android 8 and later also need the app to be allowed to install
  packages, which is granted per app; when it is not, the dialog opens that
  screen instead and says so. A download that does not match its digest is
  deleted and never reaches the installer.
- **A Linux AppImage** (the AppImage runtime names its file in `APPIMAGE`):
  the new AppImage is written next to the running one
  (`.<name>.update`), showing the megabytes done, checked the same way,
  given the running file's permissions and moved over it. The new version
  runs from the next start. Windows, macOS, the `.deb` and the tar.gz keep
  Download and the system's installer.

## What the dialog offers

- **A newer release**: Download, Release notes, Skip this version and
  Close, plus Install when the file can be put in place. Skip keeps that
  version out of the daily check.
- **No newer build**: the version that is running, and Close.
- **A failed check** (no connection, a proxy that is down): why, and Open
  the releases page.

On a phone the drawer shows the version it runs, and beside it a Check for
updates button that starts the same check and opens the same box, one tap
from the menu rather than three.

Options > Global > General, Check for updates at start (on by default):
once a day the same check runs in the background at start, and only a
newer release that was not skipped is shown.

## Checks

- `updates::tests::versions_compare_by_number`
- `updates::tests::a_development_build_is_newer_than_the_release_it_follows`
- `updates::tests::the_download_fits_the_system`
- `updates::tests::a_phone_gets_the_apk_of_its_own_abi`
- `updates::tests::a_development_release_takes_its_version_from_its_files`
- `updates::tests::a_release_with_no_file_for_this_system_is_passed_over`
- `updates::tests::drafts_are_not_offered`
- `updates::tests::no_file_of_another_system_is_offered`
- `updates::tests::the_x86_64_appimage_is_named_amd64_from_0_19_9`
- `updates::tests::how_a_linux_copy_was_installed`
- `updates::tests::the_appimage_comes_with_its_digest`
- `updates::tests::only_downloads_go_through_the_proxy`
- `updates::tests::installing_replaces_the_file_only_when_it_checks_out`
- `ui::dialogs::tests::the_updates_dialog_draws_every_state`
- `updates::tests::live_install_from_the_latest_release` (run by hand,
  needs the network: `cargo test -- --ignored`): the newest release parses
  through the proxy, its file downloads and its digest checks out.
