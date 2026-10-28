# Updates

The browser build is always the latest: there Help > Updates opens the
releases page. Every other build checks.

## Where the releases are

The project publishes from `gitshang5018/tracedraw`. Two kinds of release
live there, and both are followed:

- the development builds, as the pre-release tagged `nightly`, whose files
  are replaced on every push to main and named after the commit
  (`tracedraw-0.19.8-dev.4c864d1-android-arm64-v8a.apk`);
- a version, as a release of its own (`v0.19.9`), named without the commit.

The check reads the list of releases
(`api.github.com/repos/gitshang5018/tracedraw/releases`), newest first, and
takes the first one that carries a file for this system and is newer than
the build that is running. The list rather than `/releases/latest`: that
address never names a pre-release, and the development builds are
pre-releases. A release with nothing for this system is passed over, so a
Linux build looks past the nightlies (APKs only) to the last release that
had an AppImage.

Every GitHub address goes through a proxy, which reaches GitHub from
networks that cannot: `https://github.adc.dpdns.org/` and then the whole
GitHub address, as the proxy documents it
(`https://github.adc.dpdns.org/https://github.com/...`). That covers the
API call, the release page and the download; anything that is not a GitHub
address is left alone (`updates::through_proxy`).

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

## What the dialog offers

- **A newer release**: Download, Release notes, Skip this version and
  Close, plus Install when the file can be put in place (below). Download
  opens the file's link and the system's installer does the rest; Skip
  keeps that version out of the daily check.
- **Install in place**, when the release's file carries a SHA-256 digest
  (GitHub gives one for every uploaded file) and there is somewhere to put
  it:
  - **A phone**: the APK for the phone's own ABI (`arm64-v8a`,
    `armeabi-v7a`, `x86_64`) is downloaded into the app's cache, its size
    and digest are checked, and the file is written into a
    `PackageInstaller` session and committed, so the system's own prompt
    asks the user to update TraceDraw. Android 8 and later also need the
    app to be allowed to install packages, which is granted per app; when
    it is not, the dialog opens that screen instead and says so. A download
    that does not match its digest is deleted and never reaches the
    installer.
  - **A Linux AppImage** (the AppImage runtime names its file in
    `APPIMAGE`): the new AppImage is written next to the running one
    (`.<name>.update`), checked the same way, given the running file's
    permissions and moved over it. The new version runs from the next
    start.
- **No newer build**: the version that is running, and Close.
- **A failed check** (no connection, a proxy that is down): why, and Open
  the releases page.

Options > Global > General, Check for updates at start (on by default):
once a day the same check runs in the background at start, and only a
newer release that was not skipped is shown.

## Checks

- `updates::tests::versions_compare_by_number`
- `updates::tests::a_development_build_is_newer_than_the_release_it_follows`
- `updates::tests::the_download_fits_the_system`
- `updates::tests::a_phone_gets_the_apk_of_its_own_abi`
- `updates::tests::a_development_release_takes_its_version_from_its_files`
- `updates::tests::the_same_or_older_drafts_are_not_offered`
- `updates::tests::the_download_comes_with_its_digest`
- `updates::tests::github_addresses_go_through_the_proxy`
- `updates::tests::installing_replaces_the_file_only_when_it_checks_out`
- `ui::dialogs::tests::the_updates_dialog_draws_every_state`
- `updates::tests::live_check_finds_the_newest_release` (run by hand, needs
  the network: `cargo test -- --ignored`): the newest release parses
  through the proxy, its file downloads and its digest checks out.
