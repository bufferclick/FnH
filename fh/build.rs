//! FNH's Windows resources and embedded UI bundles:
//! - `icons/fh.ico`, author credit (`Buffer`), and Explorer version from `VERSION`
//! - `assets/FnH-Dark-Glass.zip` (universal Java + Horizon Dark-Glass resource pack)
//! - optional `assets/fnh-menu-1.21.11-1.9.0.jar` & `assets/fnh-menu-26.3-1.9.0.jar` staged by CI
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    let version = std::fs::read_to_string(format!("{dir}/VERSION"))
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    if version.is_empty() {
        panic!("{dir}/VERSION is empty: it holds the release, such as 1.9");
    }
    println!("cargo:rustc-env=FH_VERSION={version}");
    println!("cargo:rerun-if-changed={dir}/VERSION");

    for asset in [
        "FnH-Dark-Glass.zip",
        "fnh-menu-1.21.11-1.9.2.jar",
        "fnh-menu-26.3-1.9.2.jar",
    ] {
        let src = std::path::PathBuf::from(&dir).join("assets").join(asset);
        println!("cargo:rerun-if-changed={}", src.display());
        let dst = std::path::PathBuf::from(&out_dir).join(asset);
        if src.is_file() {
            std::fs::copy(&src, &dst)
                .unwrap_or_else(|e| panic!("copying {} -> {}: {e}", src.display(), dst.display()));
        } else {
            std::fs::write(&dst, b"").unwrap_or_else(|e| panic!("writing empty {}: {e}", dst.display()));
        }
    }

    let ico = format!("{dir}/icons/fh.ico");
    println!("cargo:rerun-if-changed={ico}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Resolve imports before main from System32, never a nearby injector DLL.
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&ico);
        resource.set("CompanyName", "Buffer");
        resource.set("ProductName", "FNH");
        resource.set("FileDescription", "FNH — Fabric n Horizon launcher, made by Buffer");
        resource.set("OriginalFilename", "FH.exe");
        resource.set("FileVersion", &version);
        resource.set("ProductVersion", &version);
        resource.set(
            "LegalCopyright",
            "Made by Buffer (MIT licence). Not an official Minecraft product.",
        );
        if let Err(e) = resource.compile() {
            panic!("embedding {ico}: {e}");
        }
    }
}
