// Embed the icon and version info into the Windows executable, and list
// the fonts bundled into the browser build.
fn main() {
    web_fonts();
    build_version();
    build_time();
    update_host();
    println!("cargo:rerun-if-changed=../../assets/icon/tracedraw.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/tracedraw.ico");
        res.set("ProductName", "TraceDraw");
        res.set("FileDescription", "TraceDraw vector editor");
        res.set("CompanyName", "Ighor Toth");
        res.set(
            "LegalCopyright",
            "Copyright \u{a9} 2026 Ighor Toth and the TraceDraw contributors",
        );
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}

/// The version this build reports, for the update check: a release tag
/// (`0.19.8`) or the development build of a commit (`0.19.8-dev.4c864d1`).
/// The packaging scripts pass it in, since the crate version alone cannot
/// tell two builds of the same version apart; without it the crate
/// version is the answer.
fn build_version() {
    println!("cargo:rerun-if-env-changed=TRACEDRAW_BUILD_VERSION");
    let version = match std::env::var("TRACEDRAW_BUILD_VERSION") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
    };
    println!("cargo:rustc-env=TRACEDRAW_BUILD_VERSION={version}");
}

/// Where the update files are published, when they are published anywhere
/// but the release itself (Cloudflare R2's public address, say). The
/// workflow passes it in from the repository's variables, so the address is
/// a setting rather than a line of code; without it the app downloads from
/// the release.
fn update_host() {
    println!("cargo:rerun-if-env-changed=TRACEDRAW_APK_HOST");
    let host = std::env::var("TRACEDRAW_APK_HOST").unwrap_or_default();
    println!("cargo:rustc-env=TRACEDRAW_APK_HOST={}", host.trim());
}

/// When this build was made, in seconds since 1970: the update check tells
/// a build from the one before it by this, since two development builds
/// carry a commit each and nothing says which came first.
fn build_time() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=TRACEDRAW_BUILD_TIME={now}");
}

/// The browser build has no system fonts: bundle the font files found on
/// the build machine (directories in TRACEDRAW_WEB_FONTS, separated by
/// ':', or the usual Linux font folders). Writes `web_fonts.rs` into
/// OUT_DIR; the list is empty for native builds.
fn web_fonts() {
    println!("cargo:rerun-if-env-changed=TRACEDRAW_WEB_FONTS");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    let mut entries = Vec::new();
    if wasm {
        let dirs: Vec<std::path::PathBuf> = match std::env::var("TRACEDRAW_WEB_FONTS") {
            Ok(v) if !v.is_empty() => v.split(':').map(std::path::PathBuf::from).collect(),
            _ => [
                "/usr/share/fonts/truetype/liberation",
                "/usr/share/fonts/truetype/liberation2",
                "/usr/share/fonts/truetype/dejavu",
                "/usr/share/fonts/truetype/noto",
            ]
            .iter()
            .map(std::path::PathBuf::from)
            .collect(),
        };
        let wanted = [
            "LiberationSans-Regular.ttf",
            "LiberationSans-Bold.ttf",
            "LiberationSans-Italic.ttf",
            "LiberationSans-BoldItalic.ttf",
            "LiberationSerif-Regular.ttf",
            "LiberationSerif-Bold.ttf",
            "LiberationSerif-Italic.ttf",
            "LiberationSerif-BoldItalic.ttf",
            "LiberationMono-Regular.ttf",
            "LiberationMono-Bold.ttf",
            "LiberationMono-Italic.ttf",
            "LiberationMono-BoldItalic.ttf",
            "DejaVuSans.ttf",
            "DejaVuSans-Bold.ttf",
            "NotoSansDevanagari-Regular.ttf",
            "NotoSansBengali-Regular.ttf",
        ];
        for name in wanted {
            if let Some(path) = dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
                println!("cargo:rerun-if-changed={}", path.display());
                entries.push(format!(
                    "    ({name:?}, include_bytes!({:?})),",
                    path.display().to_string()
                ));
            }
        }
        if entries.is_empty() {
            println!("cargo:warning=no fonts found for the browser build; set TRACEDRAW_WEB_FONTS");
        }
    }
    let code = format!(
        "/// Font files bundled at build time: (file name, data).\npub static WEB_FONTS: &[(&str, &[u8])] = &[\n{}\n];\n",
        entries.join("\n")
    );
    let _ = std::fs::write(out.join("web_fonts.rs"), code);
}
