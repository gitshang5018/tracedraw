//! Checking for a newer release (Help > Updates, and once a day at start
//! when Options > Global > General allows it). The check asks GitHub for
//! the releases of the project, compares the newest with the build that is
//! running and, when it is newer, offers the download for this system and
//! the release notes.
//!
//! Nothing is installed without the user, and nothing is installed from a
//! file that does not match the digest the release carries: a phone writes
//! the APK into its cache and hands it to the system's installer
//! ([`Installer`], `android_system::install_package`), a Linux AppImage
//! (run as one, so `APPIMAGE` names its file) is replaced in place, and
//! every other build opens the download's link.
//!
//! The project publishes its development builds as the pre-release
//! "nightly", replaced on every push, and a version as a release of its
//! own; both are followed, so the newest build of either is the one
//! offered. The browser build is always the latest.

use serde::Deserialize;
use std::sync::{Arc, Mutex};

/// A proxy of GitHub, for networks that cannot reach it: the address of the
/// proxy is a prefix, and the GitHub address follows it whole
/// (`https://github.adc.dpdns.org/https://github.com/...`).
pub const PROXY: &str = "https://github.adc.dpdns.org/";
/// The releases, newest first. The list rather than "latest", since
/// "latest" never names a pre-release and the development builds are
/// pre-releases.
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/gitshang5018/tracedraw/releases?per_page=10";
/// The release list, for when no download fits this system.
pub const RELEASES_PAGE: &str = "https://github.com/gitshang5018/tracedraw/releases";
/// How often the check at start runs, seconds.
pub const DAILY: u64 = 24 * 60 * 60;

/// The version this build reports: the release tag it was built from
/// (`0.19.9`), or the development build of a commit
/// (`0.19.9-dev.4c864d1`), which the packaging scripts pass in through
/// `build.rs`. Without them it is the crate version.
pub fn version() -> &'static str {
    option_env!("TRACEDRAW_BUILD_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// `url` reached through the proxy, when it is a GitHub address; anything
/// else is left as it is.
pub fn through_proxy(url: &str) -> String {
    let github = url.starts_with("https://github.com/")
        || url.starts_with("https://api.github.com/")
        || url.starts_with("https://objects.githubusercontent.com/");
    if github {
        format!("{PROXY}{url}")
    } else {
        url.to_string()
    }
}

/// A release newer than this version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// The release's page (its notes).
    pub page: String,
    /// The download for this system, when the release has one.
    pub download: Option<String>,
    /// The download with the digest the release carries, for installing in
    /// place: the APK on a phone, the AppImage on Linux.
    pub package: Option<Package>,
}

/// A release file that can be installed in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub url: String,
    /// Lower-case hex.
    pub sha256: String,
    pub size: u64,
}

/// Where a check stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    /// This version is the latest.
    Latest,
    Available(Release),
    Failed(String),
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    /// "sha256:<hex>" (GitHub computes it for every upload).
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// `1.2.3` from `v1.2.3` (pre-release parts after `-` are dropped).
fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>());
    let major = it.next()?.ok()?;
    let minor = it.next().unwrap_or(Ok(0)).ok()?;
    let patch = it.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

/// Whether `tag` names a version later than `current`.
///
/// The same version number can be built twice: a development build of a
/// commit is later than the release of that version, and two development
/// builds differ by the commit they were made from, so any difference
/// between them counts as newer. A release of a version a development
/// build already carries is never newer.
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(a), Some(b)) if a != b => a > b,
        (Some(_), Some(_)) => {
            let dev = |s: &str| s.contains("-dev.");
            match (dev(tag), dev(current)) {
                (true, false) => true,
                (true, true) => tag.trim() != current.trim(),
                (false, _) => false,
            }
        }
        _ => false,
    }
}

/// The version a release carries: its tag when that names one, else the
/// one its files are named after (`tracedraw-0.19.9-dev.4c864d1-android-
/// arm64-v8a.apk`), since a development release is published under a fixed
/// tag ("nightly") and the commit is only in the file names.
fn release_version(r: &ApiRelease) -> String {
    if parse_version(&r.tag_name).is_some() {
        return r.tag_name.trim_start_matches(['v', 'V']).to_string();
    }
    for asset in &r.assets {
        let Some(rest) = asset.name.strip_prefix("tracedraw-") else {
            continue;
        };
        for platform in ["android", "linux", "macos", "windows"] {
            if let Some(at) = rest.find(&format!("-{platform}-")) {
                let version = &rest[..at];
                if parse_version(version).is_some() {
                    return version.to_string();
                }
            }
        }
    }
    r.tag_name.clone()
}

/// How this copy of TraceDraw was installed, which decides the release
/// file that updates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Linux, run as an AppImage (`APPIMAGE` names its file).
    AppImage,
    /// Linux, installed from the release's `.deb` (dpkg lists the program).
    Deb,
    /// Linux, the program unpacked from the release's tar.gz.
    Tarball,
    /// Windows, installed with the release's setup program.
    Setup,
    /// Windows, the portable zip (the settings folder next to the program).
    Portable,
    /// macOS, the disk image's app.
    Dmg,
    /// Android, the APK packed for the phone's own ABI.
    Apk,
    /// Anything the release has no file for (an rpm or other distribution
    /// package, a sandboxed copy, the browser): no download is offered.
    Other,
}

/// The release's architecture names for `arch` (as
/// `std::env::consts::ARCH`): the one in the archive, AppImage, disk
/// image and Windows names, and the Debian one. `None` for an
/// architecture the release has no build of.
fn arch_names(arch: &str) -> Option<(&'static str, &'static str)> {
    match arch {
        "x86_64" => Some(("x86_64", "amd64")),
        "aarch64" => Some(("arm64", "arm64")),
        _ => None,
    }
}

/// The exact names of the release file of `version` that updates a copy
/// installed as `kind` on `os` (as `std::env::consts::OS`) and `arch`,
/// the preferred one first. Empty when the release builds no such file.
/// The names are the ones the release workflow gives its files. The
/// x86_64 AppImage is `linux-amd64` from 0.19.9 on, so that it sorts
/// before the arm64 one for updaters of 0.19.8 and earlier, which take
/// the first AppImage listed (#1038); older releases name it `linux-x86_64`.
pub fn asset_names(version: &str, os: &str, arch: &str, kind: Kind) -> Vec<String> {
    // A phone is packed once per ABI, and a build only runs on the one it
    // was packed for.
    if os == "android" {
        let abi = match arch {
            "aarch64" => "arm64-v8a",
            "arm" | "armv7" => "armeabi-v7a",
            "x86_64" => "x86_64",
            "x86" => "x86",
            _ => return Vec::new(),
        };
        return match kind {
            Kind::Apk => vec![format!("tracedraw-{version}-android-{abi}.apk")],
            _ => Vec::new(),
        };
    }
    let Some((a, deb)) = arch_names(arch) else {
        return Vec::new();
    };
    let one = match (os, kind) {
        ("linux", Kind::AppImage) if arch == "x86_64" => {
            return vec![
                format!("tracedraw-{version}-linux-amd64.AppImage"),
                format!("tracedraw-{version}-linux-x86_64.AppImage"),
            ];
        }
        ("linux", Kind::AppImage) => format!("tracedraw-{version}-linux-{a}.AppImage"),
        ("linux", Kind::Tarball) => format!("tracedraw-{version}-linux-{a}.tar.gz"),
        ("linux", Kind::Deb) => format!("tracedraw_{version}_{deb}.deb"),
        ("windows", Kind::Setup) => format!("tracedraw-{version}-windows-{a}-setup.exe"),
        ("windows", Kind::Portable) => format!("tracedraw-{version}-windows-{a}-portable.zip"),
        ("macos", Kind::Dmg) => format!("tracedraw-{version}-macos-{a}.dmg"),
        _ => return Vec::new(),
    };
    vec![one]
}

/// How a Linux copy was installed: run as an AppImage, inside a sandbox
/// (Flatpak, Snap), from the `.deb` (dpkg's file list for the package
/// names `exe`), from another system package (a program under `/usr`
/// that dpkg does not list, such as an rpm), else unpacked from the
/// tar.gz.
pub fn linux_kind(
    appimage: bool,
    sandboxed: bool,
    exe: &std::path::Path,
    dpkg_list: Option<&str>,
) -> Kind {
    if appimage {
        return Kind::AppImage;
    }
    if sandboxed {
        return Kind::Other;
    }
    let listed = dpkg_list.is_some_and(|l| l.lines().any(|line| std::path::Path::new(line) == exe));
    if listed {
        Kind::Deb
    } else if exe.starts_with("/usr") {
        Kind::Other
    } else {
        Kind::Tarball
    }
}

/// How the running copy was installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn current_kind() -> Kind {
    let exe = std::env::current_exe().ok();
    match std::env::consts::OS {
        "linux" => {
            let Some(exe) = exe else {
                return Kind::Other;
            };
            let dpkg = std::fs::read_to_string("/var/lib/dpkg/info/tracedraw.list").ok();
            linux_kind(
                std::env::var_os("APPIMAGE").is_some(),
                std::env::var_os("FLATPAK_ID").is_some() || std::env::var_os("SNAP").is_some(),
                &exe,
                dpkg.as_deref(),
            )
        }
        "windows" => match exe.as_deref().and_then(crate::settings::portable_dir) {
            Some(_) => Kind::Portable,
            None => Kind::Setup,
        },
        "macos" => Kind::Dmg,
        "android" => Kind::Apk,
        _ => Kind::Other,
    }
}

/// Read GitHub's answer: the newest release when it is newer than
/// `current` (drafts never count), `None` when this build is the newest.
/// The download is the release file named exactly for this system, its
/// processor and how this copy was installed (`kind`); when the release
/// has no such file it is passed over, never another processor's or
/// package's file.
///
/// GitHub lists the releases newest first, and the first one that fits and
/// is newer is the one to offer: the development builds carry a fixed tag
/// but a version of their own in their file names, so the order of the
/// list is what says which build came last.
pub fn parse_releases(
    json: &str,
    current: &str,
    os: &str,
    arch: &str,
    kind: Kind,
) -> Result<Option<Release>, String> {
    let list: Vec<ApiRelease> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    for r in list.iter().filter(|r| !r.draft) {
        let version = release_version(r);
        // A release with nothing for this system says nothing about it: the
        // development builds carry APKs only, so a Linux build looks past
        // them to the last release that had an AppImage.
        let find = |kind: Kind| {
            asset_names(&version, os, arch, kind)
                .into_iter()
                .find_map(|name| r.assets.iter().find(|a| a.name == name))
        };
        let Some(asset) = find(kind) else {
            continue;
        };
        if !is_newer(&version, current) {
            continue;
        }
        // The digest GitHub computes for every upload is what makes an
        // install in place safe: a file that does not match it is thrown
        // away instead of being run.
        let package = asset
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|hex| {
                hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) && asset.size > 0
            })
            .map(|hex| Package {
                url: through_proxy(&asset.browser_download_url),
                sha256: hex.to_ascii_lowercase(),
                size: asset.size,
            });
        return Ok(Some(Release {
            version,
            page: through_proxy(&r.html_url),
            download: Some(through_proxy(&asset.browser_download_url)),
            package,
        }));
    }
    Ok(None)
}

/// A check running in the background; its result lands in `state`.
#[derive(Debug, Clone, Default)]
pub struct Checker {
    pub state: Arc<Mutex<UpdateState>>,
}

impl Checker {
    pub fn get(&self) -> UpdateState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Start a check unless one is running; `ctx` gets a frame when it
    /// ends. In a browser the check is a fetch of the same address.
    pub fn start(&self, ctx: Option<egui::Context>) {
        if let Ok(mut s) = self.state.lock() {
            if *s == UpdateState::Checking {
                return;
            }
            *s = UpdateState::Checking;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let state = self.state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let req = crate::net::HttpRequest {
                    method: "GET",
                    url: through_proxy(RELEASES_URL),
                    headers: vec![("Accept".into(), "application/vnd.github+json".into())],
                    body: String::new(),
                    bytes: Vec::new(),
                };
                let next = match crate::net::send(req).await {
                    Ok(r) if r.status == 200 => {
                        match parse_releases(&r.body, version(), "web", "wasm32", Kind::Other) {
                            Ok(Some(r)) => UpdateState::Available(r),
                            Ok(None) => UpdateState::Latest,
                            Err(e) => UpdateState::Failed(e),
                        }
                    }
                    Ok(r) => UpdateState::Failed(format!("GitHub answered {}", r.status)),
                    Err(e) => UpdateState::Failed(e),
                };
                if let Ok(mut s) = state.lock() {
                    *s = next;
                }
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
                crate::web::request_repaint();
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let state = self.state.clone();
            std::thread::spawn(move || {
                let result = fetch().and_then(|json| {
                    parse_releases(
                        &json,
                        version(),
                        std::env::consts::OS,
                        std::env::consts::ARCH,
                        current_kind(),
                    )
                });
                let next = match result {
                    Ok(Some(r)) => UpdateState::Available(r),
                    Ok(None) => UpdateState::Latest,
                    Err(e) => UpdateState::Failed(e),
                };
                if let Ok(mut s) = state.lock() {
                    *s = next;
                }
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            });
        }
    }
}

/// Where an in-place install goes: the running AppImage's file (Linux,
/// `APPIMAGE` set by the AppImage runtime).
pub fn appimage_target() -> Option<std::path::PathBuf> {
    if std::env::consts::OS != "linux" || crate::files::WEB {
        return None;
    }
    let p = std::path::PathBuf::from(std::env::var_os("APPIMAGE")?);
    p.is_file().then_some(p)
}

/// Where an in-place install goes: the running AppImage's file, or the
/// file an update is written to before the phone's installer is given it.
pub fn install_target() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "android")]
    let target = crate::android_system::phone()
        .and_then(crate::android_system::cache_dir)
        .map(|dir| dir.join("tracedraw-update.apk"));
    #[cfg(not(target_os = "android"))]
    let target = appimage_target();
    target
}

/// Whether this build may put a new one in place by itself: a phone has to
/// be allowed to install packages, which the user grants for TraceDraw on a
/// screen of the system's; every other build can.
pub fn can_install_in_place() -> bool {
    #[cfg(target_os = "android")]
    let allowed = crate::android_system::phone()
        .map(crate::android_system::can_install_packages)
        .unwrap_or(false);
    #[cfg(not(target_os = "android"))]
    let allowed = true;
    allowed
}

/// The system's screen for allowing TraceDraw to install packages.
pub fn ask_install_permission() {
    #[cfg(target_os = "android")]
    {
        if let Some(phone) = crate::android_system::phone() {
            crate::android_system::ask_install_permission(phone);
        }
    }
}

/// Give the downloaded file to whatever puts it in place: the system's
/// installer on a phone, which asks the user to update the app.
#[cfg(target_os = "android")]
fn hand_over(path: &std::path::Path) -> Result<(), String> {
    crate::android_system::install_package(path)
}

/// A desktop build has already replaced its own file; the new one runs at
/// the next start.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn hand_over(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

/// Where an in-place install stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InstallState {
    #[default]
    Idle,
    Downloading {
        done: u64,
        total: u64,
    },
    /// The new version is in place; it runs from the next start.
    Installed(String),
    Failed(String),
}

/// Write `from` next to `target`, check its size and SHA-256 digest, give
/// it the target's permissions and move it over the target. On any
/// failure the target is left as it was and the partial file removed.
/// `progress` gets the bytes written so far.
pub fn install_file(
    from: &mut dyn std::io::Read,
    package: &Package,
    target: &std::path::Path,
    progress: &mut dyn FnMut(u64),
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("no file name")?;
    let part = target.with_file_name(format!(".{name}.update"));
    let result = (|| {
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        loop {
            let n = from.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            done += n as u64;
            if done > package.size {
                return Err("the download is larger than the release says".to_string());
            }
            hash.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            progress(done);
        }
        if done != package.size {
            return Err(format!(
                "the download stopped at {done} of {} bytes",
                package.size
            ));
        }
        let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != package.sha256 {
            return Err("the download does not match the release's digest".to_string());
        }
        out.sync_all().map_err(|e| e.to_string())?;
        drop(out);
        // A file being replaced keeps its permissions; a new one (the
        // update a phone writes into its cache) keeps what it was made
        // with.
        let perms = match std::fs::metadata(target) {
            Ok(m) => m.permissions(),
            Err(_) => std::fs::metadata(&part)
                .map_err(|e| e.to_string())?
                .permissions(),
        };
        std::fs::set_permissions(&part, perms).map_err(|e| e.to_string())?;
        std::fs::rename(&part, target).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// An in-place install running in the background.
#[derive(Debug, Clone, Default)]
pub struct Installer {
    pub state: Arc<Mutex<InstallState>>,
}

impl Installer {
    pub fn get(&self) -> InstallState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Download `package` and put it in place of `target`, unless an
    /// install is running.
    pub fn start(
        &self,
        version: String,
        package: Package,
        target: std::path::PathBuf,
        ctx: Option<egui::Context>,
    ) {
        #[cfg(target_arch = "wasm32")]
        let _ = (version, package, target, ctx);
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(mut s) = self.state.lock() {
                if matches!(*s, InstallState::Downloading { .. }) {
                    return;
                }
                *s = InstallState::Downloading {
                    done: 0,
                    total: package.size,
                };
            }
            let state = self.state.clone();
            std::thread::spawn(move || {
                let set = |next: InstallState| {
                    if let Ok(mut s) = state.lock() {
                        *s = next;
                    }
                    if let Some(ctx) = &ctx {
                        ctx.request_repaint();
                    }
                };
                let result = download(&package.url)
                    .and_then(|mut body| {
                        let mut last = 0u64;
                        install_file(&mut body, &package, &target, &mut |done| {
                            // A frame now and then, not for every block.
                            if done - last >= 512 * 1024 || done == package.size {
                                last = done;
                                set(InstallState::Downloading {
                                    done,
                                    total: package.size,
                                });
                            }
                        })
                    })
                    .and_then(|()| hand_over(&target));
                set(match result {
                    Ok(()) => InstallState::Installed(version),
                    Err(e) => InstallState::Failed(e),
                });
            });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn download(url: &str) -> Result<Box<dyn std::io::Read + Send>, String> {
    use ureq::tls::{RootCerts, TlsConfig};
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(20)))
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    // Redirects (GitHub sends downloads to its file host) are followed
    // here, each on a request of its own; the redirect's body is skipped.
    let mut url = url.to_string();
    for _ in 0..5 {
        let resp = agent
            .get(&url)
            .config()
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .header(
                "User-Agent",
                concat!("TraceDraw/", env!("CARGO_PKG_VERSION")),
            )
            .call()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if (300..400).contains(&status) {
            let next = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("a redirect without a location")?;
            url = next.to_string();
            continue;
        }
        if status != 200 {
            return Err(format!("the download answered {status}"));
        }
        return Ok(Box::new(resp.into_body().into_reader()));
    }
    Err("too many redirects".into())
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch() -> Result<String, String> {
    use ureq::tls::{RootCerts, TlsConfig};
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let mut resp = agent
        .get(&through_proxy(RELEASES_URL))
        .header(
            "User-Agent",
            concat!("TraceDraw/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_string()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files of the v0.5.0 release, named and listed in the order
    /// GitHub gave them (the arm64 Linux files come first). Each URL ends
    /// in the file's name.
    const RELEASE: &str = r#"{
        "tag_name": "v0.5.0",
        "html_url": "https://github.com/gitshang5018/tracedraw/releases/tag/v0.5.0",
        "prerelease": false,
        "draft": false,
        "assets": [
            {"name": "tracedraw-0.5.0-linux-arm64.AppImage", "browser_download_url": "https://x/tracedraw-0.5.0-linux-arm64.AppImage", "size": 7, "digest": "sha256:68e481c13fc5dc04ed6b106a9f801e1fa2689889f070a02b4fb89e8cb29c91ef"},
            {"name": "tracedraw-0.5.0-linux-arm64.tar.gz", "browser_download_url": "https://x/tracedraw-0.5.0-linux-arm64.tar.gz"},
            {"name": "tracedraw-0.5.0-linux-x86_64.AppImage", "browser_download_url": "https://x/tracedraw-0.5.0-linux-x86_64.AppImage", "size": 5, "digest": "sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"},
            {"name": "tracedraw-0.5.0-linux-x86_64.tar.gz", "browser_download_url": "https://x/tracedraw-0.5.0-linux-x86_64.tar.gz"},
            {"name": "tracedraw-0.5.0-macos-arm64.dmg", "browser_download_url": "https://x/tracedraw-0.5.0-macos-arm64.dmg"},
            {"name": "tracedraw-0.5.0-macos-x86_64.dmg", "browser_download_url": "https://x/tracedraw-0.5.0-macos-x86_64.dmg"},
            {"name": "tracedraw-0.5.0-windows-x86_64-portable.zip", "browser_download_url": "https://x/tracedraw-0.5.0-windows-x86_64-portable.zip"},
            {"name": "tracedraw-0.5.0-windows-x86_64-setup.exe", "browser_download_url": "https://x/tracedraw-0.5.0-windows-x86_64-setup.exe"},
            {"name": "tracedraw_0.5.0_amd64.deb", "browser_download_url": "https://x/tracedraw_0.5.0_amd64.deb"},
            {"name": "tracedraw_0.5.0_arm64.deb", "browser_download_url": "https://x/tracedraw_0.5.0_arm64.deb"}
        ]
    }"#;

    /// The development release that came after it: a fixed tag, the commit
    /// only in the file names, and APKs alone.
    const NIGHTLY: &str = r#"{
        "tag_name": "nightly",
        "html_url": "https://github.com/gitshang5018/tracedraw/releases/tag/nightly",
        "prerelease": true,
        "draft": false,
        "assets": [
            {"name": "tracedraw-0.5.0-dev.abc1234-android-arm64-v8a.apk", "browser_download_url": "https://x/tracedraw-0.5.0-dev.abc1234-android-arm64-v8a.apk", "size": 5, "digest": "sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"},
            {"name": "tracedraw-0.5.0-dev.abc1234-android-armeabi-v7a.apk", "browser_download_url": "https://x/tracedraw-0.5.0-dev.abc1234-android-armeabi-v7a.apk", "size": 4},
            {"name": "tracedraw-0.5.0-dev.abc1234-android-x86_64.apk", "browser_download_url": "https://x/tracedraw-0.5.0-dev.abc1234-android-x86_64.apk", "size": 3}
        ]
    }"#;

    /// The list GitHub answers with, newest first.
    fn sample() -> String {
        format!("[{NIGHTLY},{RELEASE}]")
    }

    /// The list with the version release alone in it.
    fn release_only() -> String {
        format!("[{RELEASE}]")
    }

    /// The list with every file of every release reversed, in case the
    /// order GitHub gives them is what a test relies on.
    fn reversed(json: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(json).expect("json");
        for r in v.as_array_mut().expect("releases") {
            r["assets"].as_array_mut().expect("assets").reverse();
        }
        v.to_string()
    }

    fn pick_in(json: &str, os: &str, arch: &str, kind: Kind) -> Option<String> {
        parse_releases(json, "0.4.0", os, arch, kind)
            .expect("parses")
            .expect("newer")
            .download
            .map(|u| u.trim_start_matches("https://x/").to_string())
    }

    fn pick(os: &str, arch: &str, kind: Kind) -> Option<String> {
        pick_in(&sample(), os, arch, kind)
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v0.5.0", "0.4.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.99.0"));
        assert!(!is_newer("v0.4.0", "0.4.0"));
        assert!(!is_newer("v0.3.9", "0.4.0"));
        assert!(!is_newer("nightly", "0.4.0"));
        assert!(!is_newer("v0.5.0-rc1", "0.5.0"));
    }

    #[test]
    fn a_development_build_is_newer_than_the_release_it_follows() {
        assert!(is_newer("0.19.9-dev.abc1234", "0.19.9"));
        assert!(is_newer("0.19.9-dev.def5678", "0.19.9-dev.abc1234"));
        assert!(is_newer("0.20.0-dev.abc1234", "0.19.9-dev.def5678"));
        // The release of a version the build already carries is not newer.
        assert!(!is_newer("0.19.9", "0.19.9-dev.abc1234"));
        assert!(!is_newer("0.19.9-dev.abc1234", "0.19.9-dev.abc1234"));
        assert!(!is_newer("0.19.8", "0.19.9-dev.abc1234"));
    }

    /// Every system, processor and kind of install gets exactly its own
    /// file of the release (#1038: an x86_64 AppImage took the arm64 one,
    /// listed first).
    #[test]
    fn the_download_fits_the_system() {
        use Kind::*;
        let cases = [
            (
                "linux",
                "x86_64",
                AppImage,
                "tracedraw-0.5.0-linux-x86_64.AppImage",
            ),
            (
                "linux",
                "aarch64",
                AppImage,
                "tracedraw-0.5.0-linux-arm64.AppImage",
            ),
            (
                "linux",
                "x86_64",
                Tarball,
                "tracedraw-0.5.0-linux-x86_64.tar.gz",
            ),
            (
                "linux",
                "aarch64",
                Tarball,
                "tracedraw-0.5.0-linux-arm64.tar.gz",
            ),
            ("linux", "x86_64", Deb, "tracedraw_0.5.0_amd64.deb"),
            ("linux", "aarch64", Deb, "tracedraw_0.5.0_arm64.deb"),
            (
                "windows",
                "x86_64",
                Setup,
                "tracedraw-0.5.0-windows-x86_64-setup.exe",
            ),
            (
                "windows",
                "x86_64",
                Portable,
                "tracedraw-0.5.0-windows-x86_64-portable.zip",
            ),
            ("macos", "aarch64", Dmg, "tracedraw-0.5.0-macos-arm64.dmg"),
            ("macos", "x86_64", Dmg, "tracedraw-0.5.0-macos-x86_64.dmg"),
        ];
        for (os, arch, kind, want) in cases {
            assert_eq!(
                pick(os, arch, kind).as_deref(),
                Some(want),
                "{os} {arch} {kind:?}"
            );
            // The file list's order does not matter.
            assert_eq!(
                pick_in(&reversed(&sample()), os, arch, kind).as_deref(),
                Some(want)
            );
        }
        let r = parse_releases(&sample(), "0.4.0", "linux", "x86_64", AppImage)
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0");
        assert!(r.page.ends_with("v0.5.0"));
    }

    /// A phone gets the APK packed for its own ABI, and nothing else.
    #[test]
    fn a_phone_gets_the_apk_of_its_own_abi() {
        use Kind::*;
        let cases = [
            (
                "aarch64",
                "tracedraw-0.5.0-dev.abc1234-android-arm64-v8a.apk",
            ),
            (
                "arm",
                "tracedraw-0.5.0-dev.abc1234-android-armeabi-v7a.apk",
            ),
            ("x86_64", "tracedraw-0.5.0-dev.abc1234-android-x86_64.apk"),
        ];
        for (arch, want) in cases {
            assert_eq!(pick("android", arch, Apk).as_deref(), Some(want), "{arch}");
            assert_eq!(
                pick_in(&reversed(&sample()), "android", arch, Apk).as_deref(),
                Some(want),
                "{arch}"
            );
        }
        // An architecture nothing is packed for is offered nothing, and a
        // phone takes no other kind of file.
        assert_eq!(pick("android", "riscv64", Apk), None);
        assert_eq!(pick("android", "aarch64", AppImage), None);
        assert_eq!(pick("android", "aarch64", Deb), None);
        assert_eq!(pick("linux", "aarch64", Apk), None);
        // The phone's APK comes with the digest of the release.
        let r = parse_releases(&sample(), "0.4.0", "android", "aarch64", Apk)
            .expect("parses")
            .expect("newer");
        let p = r.package.expect("apk with digest");
        assert_eq!(p.size, 5);
        assert!(p.sha256.starts_with("2cf24dba"));
    }

    /// With no file for this system, processor or install there is no
    /// download (the release page is offered instead), never another
    /// processor's or package's file.
    #[test]
    fn no_file_of_another_system_is_offered() {
        use Kind::*;
        // No builds for these processors and systems.
        assert_eq!(pick("windows", "aarch64", Setup), None);
        assert_eq!(pick("windows", "aarch64", Portable), None);
        assert_eq!(pick("linux", "riscv64", AppImage), None);
        assert_eq!(pick("linux", "x86", Tarball), None);
        assert_eq!(pick("freebsd", "x86_64", Tarball), None);
        assert_eq!(pick("web", "wasm32", Other), None);
        // No rpm or other system package is released.
        assert_eq!(pick("linux", "x86_64", Other), None);
        // A kind that belongs to another system.
        assert_eq!(pick("linux", "x86_64", Setup), None);
        assert_eq!(pick("macos", "aarch64", AppImage), None);
        assert_eq!(pick("windows", "x86_64", Dmg), None);
        // A release missing this processor's file offers none, not the
        // other processor's.
        let no_x86: String = release_only()
            .lines()
            .filter(|l| !l.contains("x86_64") && !l.contains("amd64"))
            .collect::<Vec<_>>()
            .join("\n");
        for kind in [AppImage, Tarball, Deb] {
            assert_eq!(pick_in(&no_x86, "linux", "x86_64", kind), None, "{kind:?}");
        }
        assert_eq!(pick_in(&no_x86, "macos", "x86_64", Dmg), None);
        let r = parse_releases(&no_x86, "0.4.0", "linux", "x86_64", AppImage)
            .expect("parses")
            .expect("newer");
        assert!(r.package.is_none(), "no install in place either");
        // Names that only end like this system's file do not count.
        let lookalike = release_only().replace(
            "tracedraw-0.5.0-linux-x86_64.AppImage\", \"browser",
            "tracedraw-0.5.0-linux-x86_64.AppImage.zsync\", \"browser",
        );
        assert_eq!(pick_in(&lookalike, "linux", "x86_64", AppImage), None);
    }

    /// From 0.19.9 on the x86_64 AppImage is named `linux-amd64`, which
    /// sorts before `linux-arm64`: updaters of 0.19.8 and earlier take the
    /// first AppImage of GitHub's list (sorted by name) and so get the
    /// x86_64 file. This version looks for `linux-amd64` first and still
    /// finds `linux-x86_64` in older releases (#1038).
    #[test]
    fn the_x86_64_appimage_is_named_amd64_from_0_19_9() {
        use Kind::*;
        let renamed = release_only().replace("linux-x86_64.AppImage", "linux-amd64.AppImage");
        assert_eq!(
            pick_in(&renamed, "linux", "x86_64", AppImage).as_deref(),
            Some("tracedraw-0.5.0-linux-amd64.AppImage")
        );
        let p = parse_releases(&renamed, "0.4.0", "linux", "x86_64", AppImage)
            .expect("parses")
            .expect("newer")
            .package
            .expect("appimage");
        assert_eq!(p.url, "https://x/tracedraw-0.5.0-linux-amd64.AppImage");
        assert_eq!(p.size, 5);
        // The tar.gz keeps its name; arm64 is unchanged.
        assert_eq!(
            pick_in(&renamed, "linux", "x86_64", Tarball).as_deref(),
            Some("tracedraw-0.5.0-linux-x86_64.tar.gz")
        );
        assert_eq!(
            pick_in(&renamed, "linux", "aarch64", AppImage).as_deref(),
            Some("tracedraw-0.5.0-linux-arm64.AppImage")
        );
        // With both names in a release the amd64 one wins, in any order.
        let both = renamed.replace(
            r#"{"name": "tracedraw-0.5.0-linux-x86_64.tar.gz""#,
            r#"{"name": "tracedraw-0.5.0-linux-amd64.AppImage", "browser_download_url": "https://x/tracedraw-0.5.0-linux-amd64.AppImage"},
            {"name": "tracedraw-0.5.0-linux-x86_64.tar.gz""#,
        );
        assert_eq!(
            pick_in(&both, "linux", "x86_64", AppImage).as_deref(),
            Some("tracedraw-0.5.0-linux-amd64.AppImage")
        );
        assert_eq!(
            pick_in(&reversed(&both), "linux", "x86_64", AppImage).as_deref(),
            Some("tracedraw-0.5.0-linux-amd64.AppImage")
        );
        // Names that only end like the file do not count.
        let lookalike = renamed.replace(
            "tracedraw-0.5.0-linux-amd64.AppImage\", \"browser",
            "tracedraw-0.5.0-linux-amd64.AppImage.zsync\", \"browser",
        );
        assert_eq!(pick_in(&lookalike, "linux", "x86_64", AppImage), None);
        // What an updater of 0.19.8 does: the first AppImage of the list
        // sorted by name is the x86_64 one now.
        let v: serde_json::Value = serde_json::from_str(&renamed).expect("json");
        let mut names: Vec<&str> = v[0]["assets"]
            .as_array()
            .expect("assets")
            .iter()
            .filter_map(|a| a["name"].as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(
            names.iter().find(|n| n.ends_with(".AppImage")).copied(),
            Some("tracedraw-0.5.0-linux-amd64.AppImage")
        );
        // The release script gives the x86_64 AppImage that name.
        let script = include_str!("../../../packaging/linux/build-appimage.sh");
        assert!(script.contains("NAME=amd64"), "build-appimage.sh");
        assert!(script.contains("linux-$NAME.AppImage"), "build-appimage.sh");
    }

    /// A development release takes its version from its file names, since
    /// its tag is the fixed "nightly": the commit is what tells two builds
    /// of the same version apart.
    #[test]
    fn a_development_release_takes_its_version_from_its_files() {
        let r = parse_releases(&sample(), "0.4.0", "android", "aarch64", Kind::Apk)
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0-dev.abc1234");
        // A build of that very commit is up to date, and so is the release
        // of the version it follows.
        assert_eq!(
            parse_releases(&sample(), "0.5.0-dev.abc1234", "android", "aarch64", Kind::Apk),
            Ok(None)
        );
        // A newer commit of the same version is offered.
        let r = parse_releases(&sample(), "0.5.0-dev.0000000", "android", "aarch64", Kind::Apk)
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0-dev.abc1234");
    }

    /// A release with nothing for this system is passed over: the Linux
    /// builds look past the nightlies (APKs only) to the last release that
    /// had an AppImage.
    #[test]
    fn a_release_with_no_file_for_this_system_is_passed_over() {
        let r = parse_releases(&sample(), "0.4.0", "linux", "x86_64", Kind::AppImage)
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0");
    }

    #[test]
    fn drafts_are_not_offered() {
        assert_eq!(
            parse_releases(&sample(), "0.5.0", "linux", "x86_64", Kind::AppImage),
            Ok(None)
        );
        let draft = sample().replace("\"draft\": false", "\"draft\": true");
        assert_eq!(
            parse_releases(&draft, "0.4.0", "linux", "x86_64", Kind::AppImage),
            Ok(None)
        );
        assert_eq!(
            parse_releases(&draft, "0.4.0", "android", "aarch64", Kind::Apk),
            Ok(None)
        );
        assert!(parse_releases("not json", "0.4.0", "linux", "x86_64", Kind::AppImage).is_err());
        assert!(parse_releases("{}", "0.4.0", "linux", "x86_64", Kind::AppImage).is_err());
        assert_eq!(
            parse_releases("[]", "0.4.0", "linux", "x86_64", Kind::AppImage),
            Ok(None)
        );
    }

    #[test]
    fn how_a_linux_copy_was_installed() {
        use std::path::Path;
        let list = "/.\n/usr\n/usr/bin\n/usr/bin/tracedraw\n/usr/bin/tracedraw-cli\n";
        let bin = Path::new("/usr/bin/tracedraw");
        let home = Path::new("/home/me/tracedraw/tracedraw");
        assert_eq!(linux_kind(true, false, bin, Some(list)), Kind::AppImage);
        assert_eq!(linux_kind(false, false, bin, Some(list)), Kind::Deb);
        assert_eq!(linux_kind(false, false, bin, None), Kind::Other, "rpm");
        assert_eq!(linux_kind(false, true, bin, Some(list)), Kind::Other);
        assert_eq!(linux_kind(false, false, home, Some(list)), Kind::Tarball);
        assert_eq!(linux_kind(false, false, home, None), Kind::Tarball);
        assert_eq!(
            linux_kind(
                false,
                false,
                Path::new("/usr/local/bin/tracedraw"),
                Some(list)
            ),
            Kind::Other
        );
    }

    #[test]
    fn the_appimage_comes_with_its_digest() {
        let r = parse_releases(&release_only(), "0.4.0", "linux", "x86_64", Kind::AppImage)
            .expect("parses")
            .expect("newer");
        let p = r.package.expect("appimage");
        assert_eq!(p.url, "https://x/tracedraw-0.5.0-linux-x86_64.AppImage");
        assert_eq!(p.size, 5);
        assert_eq!(
            p.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        // Not on other systems, nor without a usable digest.
        let w = parse_releases(&release_only(), "0.4.0", "windows", "x86_64", Kind::Setup)
            .expect("parses");
        assert!(w.expect("newer").package.is_none());
        // Nor for a copy that is not an AppImage.
        for kind in [Kind::Tarball, Kind::Deb, Kind::Other] {
            let t = parse_releases(&release_only(), "0.4.0", "linux", "x86_64", kind)
                .expect("parses");
            assert!(t.expect("newer").package.is_none(), "{kind:?}");
        }
        // An arm64 AppImage installs the arm64 file, with its digest.
        let arm = parse_releases(&release_only(), "0.4.0", "linux", "aarch64", Kind::AppImage)
            .expect("parses")
            .expect("newer")
            .package
            .expect("appimage");
        assert_eq!(arm.url, "https://x/tracedraw-0.5.0-linux-arm64.AppImage");
        assert_eq!(arm.size, 7);
        assert!(arm.sha256.starts_with("68e481c1"));
        let bad = release_only().replace("sha256:2CF2", "md5:2CF2");
        let b = parse_releases(&bad, "0.4.0", "linux", "x86_64", Kind::AppImage)
            .expect("parses")
            .expect("newer");
        assert!(b.package.is_none());
        assert!(b.download.is_some());
    }

    #[test]
    fn github_addresses_go_through_the_proxy() {
        assert_eq!(
            through_proxy("https://github.com/a/b/releases/download/nightly/c.apk"),
            "https://github.adc.dpdns.org/https://github.com/a/b/releases/download/nightly/c.apk"
        );
        assert_eq!(
            through_proxy("https://api.github.com/repos/a/b/releases"),
            "https://github.adc.dpdns.org/https://api.github.com/repos/a/b/releases"
        );
        // Anything else is left alone.
        assert_eq!(
            through_proxy("https://example.com/x"),
            "https://example.com/x"
        );
        // The release's page and files are reached through it too.
        let r = parse_releases(&sample(), "0.4.0", "android", "aarch64", Kind::Apk)
            .expect("parses")
            .expect("newer");
        assert!(r.page.starts_with(PROXY), "{}", r.page);
    }

    #[test]
    fn installing_replaces_the_file_only_when_it_checks_out() {
        let dir = std::env::temp_dir().join(format!("td-install-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("TraceDraw.AppImage");
        std::fs::write(&target, b"old").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let good = Package {
            url: String::new(),
            // sha256("hello")
            sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            size: 5,
        };
        let part = dir.join(".TraceDraw.AppImage.update");
        // A wrong digest, a short and a long download leave it alone.
        for (body, pkg) in [
            (&b"hellp"[..], good.clone()),
            (&b"hell"[..], good.clone()),
            (&b"hello!"[..], good.clone()),
        ] {
            let mut r = body;
            assert!(install_file(&mut r, &pkg, &target, &mut |_| {}).is_err());
            assert_eq!(std::fs::read(&target).expect("read"), b"old");
            assert!(!part.exists());
        }
        let mut seen = 0;
        let mut r = &b"hello"[..];
        install_file(&mut r, &good, &target, &mut |d| seen = d).expect("installs");
        assert_eq!(seen, 5);
        assert_eq!(std::fs::read(&target).expect("read"), b"hello");
        assert!(!part.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&target)
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        // A file that is not there yet (the update a phone writes into its
        // cache) is written as a new one.
        let fresh = dir.join("tracedraw-update.apk");
        let mut r = &b"hello"[..];
        install_file(&mut r, &good, &fresh, &mut |_| {}).expect("writes a new file");
        assert_eq!(std::fs::read(&fresh).expect("read"), b"hello");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Against GitHub through the proxy: the newest release parses and its
    /// file downloads. Run by hand (`cargo test -- --ignored`), it needs
    /// the network.
    #[test]
    #[ignore]
    fn live_install_from_the_latest_release() {
        let json = fetch().expect("releases");
        let r = parse_releases(
            &json,
            "0.0.1",
            std::env::consts::OS,
            std::env::consts::ARCH,
            current_kind(),
        )
        .expect("parses")
        .expect("newer");
        assert!(!r.version.is_empty());
        // v0.19.8 and earlier name it linux-x86_64, later ones linux-amd64.
        if let Some(p) = r.package.as_ref() {
            assert!(
                p.url.ends_with("-linux-x86_64.AppImage")
                    || p.url.ends_with("-linux-amd64.AppImage")
                    || p.url.ends_with(".apk"),
                "{}",
                p.url
            );
        }
        let p = r.package.expect("a file with a digest");
        let mut body = download(&p.url).expect("download");
        let dir = std::env::temp_dir().join(format!("td-live-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("update.bin");
        install_file(&mut body, &p, &target, &mut |_| {}).expect("installs");
        assert_eq!(std::fs::metadata(&target).expect("meta").len(), p.size);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
