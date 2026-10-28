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
/// (`0.19.8`), or the development build of a commit
/// (`0.19.8-dev.4c864d1`), which the packaging scripts pass in through
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
    /// A draft is never offered; a pre-release is, since the development
    /// builds are published as one.
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
/// one its files are named after (`tracedraw-0.19.8-dev.4c864d1-android-
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

/// The asset names that fit a system (`os` as `std::env::consts::OS`,
/// `arch` as `ARCH`), best first.
fn wanted(os: &str, arch: &str) -> Vec<String> {
    match os {
        "windows" => vec!["-setup.exe".into(), "-portable.zip".into()],
        "macos" => {
            let a = if arch == "aarch64" { "arm64" } else { "x86_64" };
            vec![format!("macos-{a}.dmg")]
        }
        "linux" => vec![".AppImage".into(), "linux-x86_64.tar.gz".into()],
        "android" => {
            // The APK is packed once per ABI, and a build only runs on the
            // one it was packed for.
            let abi = match arch {
                "aarch64" => "arm64-v8a",
                "arm" | "armv7" => "armeabi-v7a",
                "x86_64" => "x86_64",
                "x86" => "x86",
                _ => return Vec::new(),
            };
            vec![format!("-android-{abi}.apk")]
        }
        _ => Vec::new(),
    }
}

/// Read GitHub's answer: the newest release when it is newer than
/// `current` (drafts never count), `None` when this build is the newest.
///
/// GitHub lists the releases newest first, and the first one that is newer
/// is the one to offer: the development builds carry a fixed tag but a
/// version of their own in their file names, so the order of the list is
/// what says which build came last.
pub fn parse_releases(
    json: &str,
    current: &str,
    os: &str,
    arch: &str,
) -> Result<Option<Release>, String> {
    let list: Vec<ApiRelease> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let ends = wanted(os, arch);
    for r in list.iter().filter(|r| !r.draft) {
        // A release with nothing for this system says nothing about it: the
        // development builds carry APKs only, so a Linux build looks past
        // them to the last release that had an AppImage.
        let Some(asset) = ends
            .iter()
            .find_map(|end| r.assets.iter().find(|a| a.name.ends_with(end.as_str())))
        else {
            continue;
        };
        let version = release_version(r);
        if !is_newer(&version, current) {
            continue;
        }
        let download = Some(through_proxy(&asset.browser_download_url));
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
            download,
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
                        match parse_releases(&r.body, version(), "web", "wasm32") {
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
/// `APPIMAGE` set by the AppImage runtime), or the file an update is
/// written to before the phone's installer is given it.
pub fn install_target() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "android")]
    let target = crate::android_system::phone()
        .and_then(crate::android_system::cache_dir)
        .map(|dir| dir.join("tracedraw-update.apk"));
    #[cfg(not(target_os = "android"))]
    let target = appimage_target();
    target
}

/// The AppImage this program was started from, when it was.
#[cfg(not(target_os = "android"))]
fn appimage_target() -> Option<std::path::PathBuf> {
    if std::env::consts::OS != "linux" || crate::files::WEB {
        return None;
    }
    let p = std::path::PathBuf::from(std::env::var_os("APPIMAGE")?);
    p.is_file().then_some(p)
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

    /// A version release, and the development release that came after it.
    const SAMPLE: &str = r#"[
      {
        "tag_name": "nightly",
        "html_url": "https://github.com/gitshang5018/tracedraw/releases/tag/nightly",
        "prerelease": true,
        "draft": false,
        "assets": [
            {"name": "tracedraw-0.5.0-dev.abc1234-android-arm64-v8a.apk", "browser_download_url": "https://github.com/x/a-arm64.apk", "size": 5, "digest": "sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"},
            {"name": "tracedraw-0.5.0-dev.abc1234-android-armeabi-v7a.apk", "browser_download_url": "https://github.com/x/a-v7a.apk", "size": 4},
            {"name": "tracedraw-0.5.0-dev.abc1234-android-x86_64.apk", "browser_download_url": "https://github.com/x/a-x86.apk", "size": 3}
        ]
      },
      {
        "tag_name": "v0.5.0",
        "html_url": "https://github.com/gitshang5018/tracedraw/releases/tag/v0.5.0",
        "prerelease": false,
        "draft": false,
        "assets": [
            {"name": "tracedraw-0.5.0-linux-x86_64.AppImage", "browser_download_url": "https://github.com/x/a.AppImage", "size": 5, "digest": "sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"},
            {"name": "tracedraw-0.5.0-linux-x86_64.tar.gz", "browser_download_url": "https://github.com/x/a.tar.gz"},
            {"name": "tracedraw-0.5.0-macos-arm64.dmg", "browser_download_url": "https://github.com/x/arm.dmg"},
            {"name": "tracedraw-0.5.0-macos-x86_64.dmg", "browser_download_url": "https://github.com/x/intel.dmg"},
            {"name": "tracedraw-0.5.0-windows-x86_64-portable.zip", "browser_download_url": "https://github.com/x/p.zip"},
            {"name": "tracedraw-0.5.0-windows-x86_64-setup.exe", "browser_download_url": "https://github.com/x/setup.exe"},
            {"name": "tracedraw-0.5.0-android-arm64-v8a.apk", "browser_download_url": "https://github.com/x/b-arm64.apk", "size": 6, "digest": "sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"}
        ]
      }
    ]"#;

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
        assert!(is_newer("0.19.8-dev.abc1234", "0.19.8"));
        assert!(is_newer("0.19.8-dev.def5678", "0.19.8-dev.abc1234"));
        assert!(is_newer("0.19.9-dev.abc1234", "0.19.8-dev.def5678"));
        // The release of a version the build already carries is not newer.
        assert!(!is_newer("0.19.8", "0.19.8-dev.abc1234"));
        assert!(!is_newer("0.19.8-dev.abc1234", "0.19.8-dev.abc1234"));
        assert!(!is_newer("0.19.7", "0.19.8-dev.abc1234"));
    }

    #[test]
    fn the_download_fits_the_system() {
        let pick = |os, arch| {
            parse_releases(SAMPLE, "0.4.0", os, arch)
                .expect("parses")
                .expect("newer")
                .download
        };
        assert_eq!(
            pick("windows", "x86_64").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/setup.exe")
        );
        assert_eq!(
            pick("macos", "aarch64").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/arm.dmg")
        );
        assert_eq!(
            pick("macos", "x86_64").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/intel.dmg")
        );
        assert_eq!(
            pick("linux", "x86_64").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/a.AppImage")
        );
        assert_eq!(pick("freebsd", "x86_64"), None);
        let r = parse_releases(SAMPLE, "0.4.0", "linux", "x86_64")
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0");
        assert!(r.page.ends_with("v0.5.0"));
    }

    #[test]
    fn a_phone_gets_the_apk_of_its_own_abi() {
        let pick = |arch| {
            parse_releases(SAMPLE, "0.4.0", "android", arch)
                .expect("parses")
                .expect("newer")
                .download
        };
        assert_eq!(
            pick("aarch64").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/a-arm64.apk")
        );
        assert_eq!(
            pick("arm").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/a-v7a.apk")
        );
        assert_eq!(
            pick("x86_64").as_deref(),
            Some("https://github.adc.dpdns.org/https://github.com/x/a-x86.apk")
        );
        // An architecture nothing is packed for is offered nothing.
        assert_eq!(pick("riscv64"), None);
    }

    #[test]
    fn a_development_release_takes_its_version_from_its_files() {
        // The tag is "nightly": the commit is only in the file names.
        let r = parse_releases(SAMPLE, "0.4.0", "android", "aarch64")
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0-dev.abc1234");
        // And a build of that very commit is up to date.
        assert_eq!(
            parse_releases(SAMPLE, "0.5.0-dev.abc1234", "android", "aarch64"),
            Ok(None)
        );
        // A newer commit of the same version is offered.
        let r = parse_releases(SAMPLE, "0.5.0-dev.0000000", "android", "aarch64")
            .expect("parses")
            .expect("newer");
        assert_eq!(r.version, "0.5.0-dev.abc1234");
    }

    #[test]
    fn the_same_or_older_drafts_are_not_offered() {
        assert_eq!(parse_releases(SAMPLE, "0.5.0", "linux", "x86_64"), Ok(None));
        let draft = SAMPLE.replace("\"draft\": false", "\"draft\": true");
        assert_eq!(parse_releases(&draft, "0.4.0", "linux", "x86_64"), Ok(None));
        assert!(parse_releases("not json", "0.4.0", "linux", "x86_64").is_err());
        assert!(parse_releases("{}", "0.4.0", "linux", "x86_64").is_err());
        assert_eq!(parse_releases("[]", "0.4.0", "linux", "x86_64"), Ok(None));
    }

    #[test]
    fn the_download_comes_with_its_digest() {
        let r = parse_releases(SAMPLE, "0.4.0", "linux", "x86_64")
            .expect("parses")
            .expect("newer");
        let p = r.package.expect("package");
        assert_eq!(p.url, "https://github.adc.dpdns.org/https://github.com/x/a.AppImage");
        assert_eq!(p.size, 5);
        assert_eq!(
            p.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        // The phone's APK comes with one too.
        let a = parse_releases(SAMPLE, "0.4.0", "android", "aarch64")
            .expect("parses")
            .expect("newer");
        assert_eq!(a.package.expect("apk").size, 5);
        // A file without a usable digest is offered as a link only.
        let bad = SAMPLE.replace("sha256:2CF2", "md5:2CF2");
        let b = parse_releases(&bad, "0.4.0", "linux", "x86_64")
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
        assert_eq!(through_proxy("https://example.com/x"), "https://example.com/x");
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
        // A file that is not there yet is written as a new one.
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
    fn live_check_finds_the_newest_release() {
        let json = fetch().expect("releases");
        let r = parse_releases(
            &json,
            "0.0.1",
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
        .expect("parses")
        .expect("newer");
        assert!(!r.version.is_empty());
        let p = r.package.expect("a package with a digest");
        let mut body = download(&p.url).expect("download");
        let dir = std::env::temp_dir().join(format!("td-live-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("update.bin");
        install_file(&mut body, &p, &target, &mut |_| {}).expect("installs");
        assert_eq!(std::fs::metadata(&target).expect("meta").len(), p.size);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
