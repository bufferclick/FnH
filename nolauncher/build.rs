//! NoLauncherHorizon's Windows resources: the icon (`icons/nlh.ico`, drawn by
//! `icons/make_icon.py` from `icons/nlh.svg`) and the version Explorer shows, both from
//! `VERSION` so the .exe, `--version` and the release tag (`nlh-v1.0`) cannot drift apart.
//!
//! `VERSION` also becomes `NLH_VERSION`, which `main.rs` reads: one place to bump an update.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let version = std::fs::read_to_string(format!("{dir}/VERSION"))
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    if version.is_empty() {
        panic!("{dir}/VERSION is empty: it holds the release, such as 1.0");
    }
    println!("cargo:rustc-env=NLH_VERSION={version}");
    println!("cargo:rerun-if-changed={dir}/VERSION");
    let ico = format!("{dir}/icons/nlh.ico");
    println!("cargo:rerun-if-changed={ico}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Resolve imports before main from System32, never a nearby injector DLL (#13108).
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&ico);
        resource.set("ProductName", "NoLauncherHorizon");
        resource.set("FileDescription", "NoLauncherHorizon: Horizon without the launcher");
        resource.set("OriginalFilename", "NoLauncherHorizon.exe");
        resource.set("FileVersion", &version);
        resource.set("ProductVersion", &version);
        resource.set(
            "LegalCopyright",
            "MIT licence; Horizon is not an official Minecraft product (Mojang/Microsoft)",
        );
        if let Err(e) = resource.compile() {
            panic!("embedding {ico}: {e}");
        }
    }
}
