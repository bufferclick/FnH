//! FNH's Windows resources: the icon (`icons/fh.ico`, drawn from `icons/fh.svg`), author credit
//! (`Buffer`), and the version Explorer shows, read from `VERSION` so `FH.exe`, `--version` and
//! the release tag (`fnh-v1.6`) stay in lockstep.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let version = std::fs::read_to_string(format!("{dir}/VERSION"))
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    if version.is_empty() {
        panic!("{dir}/VERSION is empty: it holds the release, such as 1.6");
    }
    println!("cargo:rustc-env=FH_VERSION={version}");
    println!("cargo:rerun-if-changed={dir}/VERSION");
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
