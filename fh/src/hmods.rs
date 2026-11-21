//! Horizon mod subsystem for FNH (`v1.7`), aligned with upstream `horizon-rust/horizon` (`v0.2`):
//!
//! 1. **Unified `.hmod` packages (`horizon-mod.toml`)**:
//!    - **Content mods** (`assets/`, `data/`, packs): installed to `<home>/horizon-mods/<id>.hmod`
//!      (or `<home>/horizon-mods/<id>/` when ported from a Java `.jar`), loaded directly by
//!      `horizon-client.exe` at startup with zero compilation.
//!    - **WASM plugins** (`[wasm]` + `mod.wasm` verified with `\0asm` magic & optional SHA-512):
//!      installed to `<home>/horizon-mods/<id>.hmod`, executed inside Horizon's built-in sandboxed
//!      WebAssembly runtime with zero external compiler required.
//!    - **Native Rust mods** (`[native]` + `Cargo.toml`, or `.hnmod`): installed to
//!      `<home>/horizon-native-mods/<id>.hmod` and linked via Horizon's official Native Mod SDK
//!      (`horizon-launcher native-build`) before launch.
//! 2. **Horizon Registry (`https://horizon-rust.github.io/horizon-mods/index.json`)**:
//!    - Supports both `api = "0.1"` and `api = "0.2"` indexes (`mod`, `native-mod` as `.hmod` or
//!      `.hnmod`, `resourcepack`, `shader`, `shaderpack`, `modpack`), checks `revoked` takedowns,
//!      and verifies exact byte size, `SHA-256`, and `minisign` author signatures.
//! 3. **GitHub Mod Installer**:
//!    - Installs Horizon mods directly from `https://github.com/<owner>/<repo>` (latest or tagged
//!      `.hmod` release asset, or source repository containing `horizon-mod.toml`).
//! 4. **Manifest-keyed `horizon-mods.txt` state**:
//!    - Reads and updates `mod.<manifest_id>:true|false` in `<home>/horizon-mods.txt` so FNH,
//!      `horizon-client`, and `horizon-launcher native-build` agree on every mod's enabled state.

use crate::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Live Horizon Mod Registry index URL (`horizon-rust/horizon-mods` GitHub Pages).
pub const REGISTRY_INDEX_URL: &str = "https://horizon-rust.github.io/horizon-mods/index.json";
/// Releases API for upstream `horizon-rust/horizon` (used to fetch `horizon-launcher` for Native Mod SDK builds).
pub const HORIZON_RELEASES_API: &str =
    "https://api.github.com/repos/horizon-rust/horizon/releases?per_page=25";

/// Runtime execution model of a Horizon mod package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HorizonModRuntime {
    /// Data / assets / ported Modrinth pack — runs out of the box in `horizon-client`.
    Content,
    /// Sandboxed WebAssembly plugin (`[wasm]` + `mod.wasm`) — runs out of the box in `horizon-client`.
    Wasm,
    /// Native Rust source mod (`[native]` + `Cargo.toml`) — linked via Native Mod SDK (`native-build`).
    Native,
}

impl HorizonModRuntime {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Content => "Content",
            Self::Wasm => "WASM",
            Self::Native => "Native SDK",
        }
    }
}

/// Parsed metadata from a `.hmod` / `.hnmod` archive or unpacked Horizon mod directory.
#[derive(Clone, Debug)]
pub struct InspectedHorizonMod {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api: String,
    pub description: String,
    pub authors: Vec<String>,
    pub runtime: HorizonModRuntime,
    /// Required mod IDs -> version requirement string from `[dependencies]`.
    pub dependencies: BTreeMap<String, String>,
    /// Ported Java compatibility note from `mod-browser.json`, if this folder came from `horizon-port-pack`.
    pub ported_note: Option<String>,
    /// Unhandled Java classes/systems noted by `horizon-port-pack`, if any.
    pub ported_unhandled: Vec<String>,
}

#[derive(Deserialize, Default)]
struct RawHorizonManifest {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    api: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
    #[serde(default)]
    wasm: Option<RawWasmSection>,
    #[serde(default)]
    native: Option<toml::Value>,
}

#[derive(Deserialize, Default)]
struct RawWasmSection {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Deserialize, Default)]
struct PortedMetaJson {
    #[serde(default)]
    mod_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    compat: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    unhandled: Vec<String>,
}

/// Validates a Horizon mod ID (`[a-z0-9][a-z0-9_-]{0,63}`).
pub fn valid_mod_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
}

/// Sanitizes an arbitrary stem into a safe Horizon mod ID.
pub fn sanitize_mod_id(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if (c == '-' || c == '_' || c == ' ' || c == '.') && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
        if out.len() >= 64 {
            break;
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "mod".to_owned()
    } else if trimmed.as_bytes()[0].is_ascii_lowercase() || trimmed.as_bytes()[0].is_ascii_digit() {
        trimmed
    } else {
        format!("m_{trimmed}")
    }
}

fn parse_dependencies(raw: &BTreeMap<String, toml::Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        let req = match v {
            toml::Value::String(s) => s.clone(),
            toml::Value::Table(t) => t
                .get("version")
                .and_then(|x| x.as_str())
                .unwrap_or("*")
                .to_owned(),
            _ => "*".to_owned(),
        };
        out.insert(k.clone(), req);
    }
    out
}

/// Inspects a `.hmod` or `.hnmod` zip archive, validating `horizon-mod.toml`, WASM magic/SHA-512,
/// or Native `Cargo.toml`.
pub fn inspect_hmod_archive(path: &Path) -> Result<InspectedHorizonMod, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let file =
        fs::File::open(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("{} is not a valid .hmod archive: {e}", path.display()))?;

    let mut manifest_text: Option<String> = None;
    let mut has_cargo_toml = false;
    let mut has_horizon_native_json = false;
    let mut entry_names = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt zip entry in {}: {e}", path.display()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|e| format!("Invalid zip path in {}: {e}", path.display()))?
            .replace('\\', "/");
        if name.contains("..") || name.starts_with('/') {
            return Err(format!("Unsafe path '{name}' inside {}", path.display()));
        }
        if name == "horizon-mod.toml" {
            let mut s = String::new();
            entry
                .read_to_string(&mut s)
                .map_err(|e| format!("Could not read horizon-mod.toml: {e}"))?;
            manifest_text = Some(s);
        } else if name == "Cargo.toml" {
            has_cargo_toml = true;
        } else if name == "horizon-native.json" {
            has_horizon_native_json = true;
        }
        entry_names.push(name);
    }

    let fallback_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("mod");

    let raw: RawHorizonManifest = match manifest_text {
        Some(ref txt) => toml::from_str(txt)
            .map_err(|e| format!("Invalid horizon-mod.toml in {}: {e}", path.display()))?,
        None if ext == "hnmod" || has_horizon_native_json || has_cargo_toml => RawHorizonManifest {
            id: sanitize_mod_id(fallback_stem),
            name: fallback_stem.to_owned(),
            version: "0.1.0".into(),
            api: "0.2".into(),
            ..Default::default()
        },
        None => {
            return Err(format!(
                "{} is missing horizon-mod.toml at the archive root.",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Archive")
            ));
        }
    };

    let id = if valid_mod_id(raw.id.trim()) {
        raw.id.trim().to_owned()
    } else {
        sanitize_mod_id(if raw.id.trim().is_empty() {
            fallback_stem
        } else {
            raw.id.trim()
        })
    };

    let runtime = if ext == "hnmod" || raw.native.is_some() || has_horizon_native_json {
        HorizonModRuntime::Native
    } else if let Some(ref wasm_sec) = raw.wasm {
        let wasm_file = wasm_sec
            .file
            .as_deref()
            .unwrap_or("mod.wasm")
            .trim_start_matches("./");
        let mut entry = archive.by_name(wasm_file).map_err(|_| {
            format!(
                "WASM mod '{}' declares [wasm] but '{}' is missing from the .hmod archive.",
                id, wasm_file
            )
        })?;
        let mut wasm_bytes = Vec::new();
        entry
            .read_to_end(&mut wasm_bytes)
            .map_err(|e| format!("Could not read {wasm_file}: {e}"))?;
        if wasm_bytes.len() < 8 || &wasm_bytes[..4] != b"\0asm" {
            return Err(format!(
                "'{wasm_file}' inside '{id}' does not have a valid WebAssembly (\\0asm) header."
            ));
        }
        if let Some(expected_512) = wasm_sec
            .sha512
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let actual_512 = hex_encode(&Sha512::digest(&wasm_bytes));
            if !actual_512.eq_ignore_ascii_case(expected_512) {
                return Err(format!(
                    "SHA-512 mismatch for '{wasm_file}' inside '{id}'."
                ));
            }
        }
        HorizonModRuntime::Wasm
    } else if entry_names.iter().any(|n| n == "mod.wasm") {
        HorizonModRuntime::Wasm
    } else if has_cargo_toml {
        HorizonModRuntime::Native
    } else {
        HorizonModRuntime::Content
    };

    let name = if raw.name.trim().is_empty() {
        id.clone()
    } else {
        raw.name.trim().to_owned()
    };
    let version = if raw.version.trim().is_empty() {
        "0.1.0".to_owned()
    } else {
        raw.version.trim().to_owned()
    };
    let api = if raw.api.trim().is_empty() {
        "0.2".to_owned()
    } else {
        raw.api.trim().to_owned()
    };

    Ok(InspectedHorizonMod {
        id,
        name,
        version,
        api,
        description: raw.description.trim().to_owned(),
        authors: raw.authors,
        runtime,
        dependencies: parse_dependencies(&raw.dependencies),
        ported_note: None,
        ported_unhandled: Vec::new(),
    })
}

/// Inspects an unpacked Horizon mod folder (`<home>/horizon-mods/<dir>` or `<home>/horizon-native-mods/<dir>`).
pub fn inspect_mod_dir(dir: &Path, in_native_folder: bool) -> Result<InspectedHorizonMod, String> {
    let fallback_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mod");

    let ported_meta: Option<PortedMetaJson> = fs::read_to_string(dir.join("mod-browser.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let manifest_path = dir.join("horizon-mod.toml");
    if manifest_path.is_file() {
        let txt = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Could not read {}: {e}", manifest_path.display()))?;
        let raw: RawHorizonManifest = toml::from_str(&txt)
            .map_err(|e| format!("Invalid {}: {e}", manifest_path.display()))?;
        let id = if valid_mod_id(raw.id.trim()) {
            raw.id.trim().to_owned()
        } else {
            sanitize_mod_id(fallback_name)
        };
        let runtime = if in_native_folder
            || raw.native.is_some()
            || dir.join("Cargo.toml").is_file()
            || dir.join("horizon-native.json").is_file()
        {
            HorizonModRuntime::Native
        } else if raw.wasm.is_some() || dir.join("mod.wasm").is_file() {
            HorizonModRuntime::Wasm
        } else {
            HorizonModRuntime::Content
        };
        let (ported_note, ported_unhandled) = match ported_meta {
            Some(pm) => {
                let note = if !pm.summary.is_empty() {
                    Some(pm.summary)
                } else if !pm.compat.is_empty() {
                    Some(format!("Ported Java mod ({})", pm.compat))
                } else {
                    Some("Ported from Java .jar".into())
                };
                (note, pm.unhandled)
            }
            None => (None, Vec::new()),
        };
        return Ok(InspectedHorizonMod {
            name: if raw.name.trim().is_empty() {
                ported_meta_title(fallback_name, None)
            } else {
                raw.name.trim().to_owned()
            },
            version: if raw.version.trim().is_empty() {
                "0.1.0".into()
            } else {
                raw.version.trim().to_owned()
            },
            api: if raw.api.trim().is_empty() {
                "0.2".into()
            } else {
                raw.api.trim().to_owned()
            },
            description: raw.description.trim().to_owned(),
            authors: raw.authors,
            runtime,
            dependencies: parse_dependencies(&raw.dependencies),
            id,
            ported_note,
            ported_unhandled,
        });
    }

    if let Some(pm) = ported_meta {
        let id = if valid_mod_id(&pm.mod_id) {
            pm.mod_id.clone()
        } else {
            sanitize_mod_id(fallback_name)
        };
        return Ok(InspectedHorizonMod {
            id,
            name: if pm.title.trim().is_empty() {
                fallback_name.to_owned()
            } else {
                pm.title
            },
            version: if pm.version.trim().is_empty() {
                "ported".into()
            } else {
                pm.version
            },
            api: "0.2".into(),
            description: pm.summary.clone(),
            authors: Vec::new(),
            runtime: HorizonModRuntime::Content,
            dependencies: BTreeMap::new(),
            ported_note: Some(if pm.summary.is_empty() {
                "Ported from Java .jar".into()
            } else {
                pm.summary
            }),
            ported_unhandled: pm.unhandled,
        });
    }

    if in_native_folder
        || dir.join("Cargo.toml").is_file()
        || dir.join("horizon-native.json").is_file()
        || dir.join("pack.mcmeta").is_file()
        || dir.join("assets").is_dir()
        || dir.join("data").is_dir()
    {
        let runtime = if in_native_folder
            || dir.join("Cargo.toml").is_file()
            || dir.join("horizon-native.json").is_file()
        {
            HorizonModRuntime::Native
        } else {
            HorizonModRuntime::Content
        };
        return Ok(InspectedHorizonMod {
            id: sanitize_mod_id(fallback_name),
            name: fallback_name.to_owned(),
            version: "local".into(),
            api: "0.2".into(),
            description: String::new(),
            authors: Vec::new(),
            runtime,
            dependencies: BTreeMap::new(),
            ported_note: None,
            ported_unhandled: Vec::new(),
        });
    }

    Err(format!(
        "Folder '{}' does not contain horizon-mod.toml, Cargo.toml, or assets/data.",
        dir.display()
    ))
}

fn ported_meta_title(fallback: &str, title: Option<&str>) -> String {
    title
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

/// Inspects either a `.hmod`/`.hnmod` archive or a directory.
pub fn inspect_any_mod_path(path: &Path, in_native_folder: bool) -> Result<InspectedHorizonMod, String> {
    if path.is_dir() {
        inspect_mod_dir(path, in_native_folder)
    } else {
        inspect_hmod_archive(path)
    }
}

/// Reads `<home>/horizon-mods.txt` into a map of `mod_id -> enabled`.
pub fn read_mods_state(home: &Path) -> BTreeMap<String, bool> {
    let mut map = BTreeMap::new();
    let Ok(text) = fs::read_to_string(home.join("horizon-mods.txt")) else {
        return map;
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("mod.") {
            if let Some((id, val)) = rest.split_once(':') {
                let enabled = !matches!(
                    val.trim().to_ascii_lowercase().as_str(),
                    "false" | "0" | "off" | "no"
                );
                map.insert(id.trim().to_owned(), enabled);
            }
        }
    }
    map
}

/// Writes `mod.<mod_id>:<enabled>` into `<home>/horizon-mods.txt` while preserving comments and all
/// other settings in the file.
pub fn set_mod_state(home: &Path, mod_id: &str, enabled: bool) -> Result<(), String> {
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let path = home.join("horizon-mods.txt");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let prefix = format!("mod.{mod_id}:");
    let mut replaced = false;
    let mut lines: Vec<String> = existing
        .lines()
        .map(|l| {
            if l.trim_start().starts_with(&prefix) {
                replaced = true;
                format!("mod.{mod_id}:{enabled}")
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !replaced {
        lines.push(format!("mod.{mod_id}:{enabled}"));
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    fs::write(&path, out).map_err(|e| format!("Could not update horizon-mods.txt: {e}"))
}

/// Removes `mod.<mod_id>:*` from `<home>/horizon-mods.txt`.
pub fn remove_mod_state(home: &Path, mod_id: &str) {
    let path = home.join("horizon-mods.txt");
    let Ok(existing) = fs::read_to_string(&path) else {
        return;
    };
    let prefix = format!("mod.{mod_id}:");
    let kept: Vec<&str> = existing
        .lines()
        .filter(|l| !l.trim_start().starts_with(&prefix))
        .collect();
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    let _ = fs::write(&path, out);
}

/// Installs a local `.hmod`, `.hnmod`, or unpacked mod folder into `home`, routing Content/WASM
/// packages to `horizon-mods/` and Native SDK packages to `horizon-native-mods/`, and marks
/// `mod.<id>:true` in `horizon-mods.txt`.
pub fn install_local_horizon_mod(home: &Path, src: &Path) -> Result<InspectedHorizonMod, String> {
    let inspected = inspect_any_mod_path(src, false)?;
    let target_dir = match inspected.runtime {
        HorizonModRuntime::Content | HorizonModRuntime::Wasm => home.join("horizon-mods"),
        HorizonModRuntime::Native => home.join("horizon-native-mods"),
    };
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    // Clean up any older copy of the same mod ID across active/disabled folders so there's never a duplicate.
    remove_existing_copies_by_id(home, &inspected.id);

    if src.is_dir() {
        let dst = target_dir.join(&inspected.id);
        copy_dir_recursive(src, &dst)?;
    } else {
        let ext = if src
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("hnmod"))
        {
            "hnmod"
        } else {
            "hmod"
        };
        let dst = target_dir.join(format!("{}.{ext}", inspected.id));
        fs::copy(src, &dst).map_err(|e| format!("Could not copy mod into {}: {e}", dst.display()))?;
    }

    set_mod_state(home, &inspected.id, true)?;
    Ok(inspected)
}

fn remove_existing_copies_by_id(home: &Path, mod_id: &str) {
    for sub in ["horizon-mods", "horizon-native-mods", "horizon-mods-disabled"] {
        let dir = home.join(sub);
        for cand in [
            dir.join(format!("{mod_id}.hmod")),
            dir.join(format!("{mod_id}.hnmod")),
            dir.join(mod_id),
        ] {
            if cand.is_dir() {
                let _ = fs::remove_dir_all(&cand);
            } else if cand.is_file() {
                let _ = fs::remove_file(&cand);
            }
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        let _ = fs::remove_dir_all(dst);
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Horizon Registry (`index.json` v0.1 & v0.2)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryIndex {
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub projects: Vec<RegistryProject>,
    #[serde(default)]
    pub revoked: Vec<RegistryRevoked>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryProject {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub stars: u64,
    #[serde(default)]
    pub versions: Vec<RegistryVersion>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub game: String,
    /// Upstream v0.2 Native Mod SDK slice (`"native-1/..."`) when applicable.
    #[serde(default)]
    pub native: Option<String>,
    pub file: RegistryFile,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryRevoked {
    pub id: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub reason: String,
}

static REGISTRY_CACHE: OnceLock<Mutex<Option<(Instant, Arc<RegistryIndex>)>>> = OnceLock::new();

fn registry_url() -> String {
    std::env::var("HORIZON_MOD_INDEX")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| REGISTRY_INDEX_URL.to_owned())
}

/// Fetches (or returns the 10-minute / disk-cached) Horizon Mod Registry index (`api = "0.1"` or `"0.2"`).
pub fn fetch_registry_index() -> Result<Arc<RegistryIndex>, String> {
    let lock = REGISTRY_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = lock.lock() {
        if let Some((at, idx)) = guard.as_ref() {
            if at.elapsed() < Duration::from_secs(600) {
                return Ok(idx.clone());
            }
        }
    }

    let cache_file = paths::cache_dir().join("horizon-registry-index.json");
    let url = registry_url();

    let fetched_text = if url.starts_with("https://") {
        let agent = http_agent(25);
        agent
            .get(&url)
            .set("User-Agent", &format!("FNH/{}", crate::VERSION))
            .call()
            .map_err(|e| format!("Horizon Registry unreachable: {e}"))
            .and_then(|resp| {
                resp.into_string()
                    .map_err(|e| format!("Failed reading Horizon Registry index: {e}"))
            })
    } else {
        fs::read_to_string(&url).map_err(|e| format!("Could not read local registry {url}: {e}"))
    };

    let text = match fetched_text {
        Ok(t) => {
            if let Some(parent) = cache_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&cache_file, &t);
            t
        }
        Err(net_err) => fs::read_to_string(&cache_file).map_err(|_| net_err)?,
    };

    let index = parse_registry_index(&text)?;
    let arc = Arc::new(index);
    if let Ok(mut guard) = lock.lock() {
        *guard = Some((Instant::now(), arc.clone()));
    }
    Ok(arc)
}

/// Parses a Horizon Registry `index.json` (`api = "0.1"` or `"0.2"`).
pub fn parse_registry_index(text: &str) -> Result<RegistryIndex, String> {
    let idx: RegistryIndex = serde_json::from_str(text)
        .map_err(|e| format!("Could not parse Horizon Registry index.json: {e}"))?;
    if !matches!(idx.api.as_str(), "" | "0.1" | "0.2" | "0.3") {
        return Err(format!(
            "Horizon Registry index uses newer schema api='{}' — update FNH to browse it.",
            idx.api
        ));
    }
    Ok(idx)
}

impl RegistryIndex {
    pub fn is_revoked(&self, project_id: &str, sha256: Option<&str>) -> Option<&str> {
        for rev in &self.revoked {
            if rev.id == project_id {
                match (&rev.sha256, sha256) {
                    (None, _) => return Some(rev.reason.as_str()),
                    (Some(r_hash), Some(f_hash)) if r_hash.eq_ignore_ascii_case(f_hash) => {
                        return Some(rev.reason.as_str());
                    }
                    _ => {}
                }
            }
        }
        None
    }
}

fn allowed_registry_download_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://github.com/")
        || lower.starts_with("https://objects.githubusercontent.com/")
        || lower.starts_with("https://release-assets.githubusercontent.com/")
        || lower.starts_with("https://raw.githubusercontent.com/")
        || lower.starts_with("https://codeload.github.com/")
        || lower.starts_with("https://horizon-rust.github.io/")
}

/// Downloads, cryptographically verifies (`size` + `SHA-256` + `minisign` + takedown check), and
/// installs a project from the Horizon Mod Registry into `home`.
pub fn install_registry_project(home: &Path, project_id: &str) -> Result<String, String> {
    let index = fetch_registry_index()?;
    let proj = index
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| format!("Project '{project_id}' was not found in the Horizon Registry."))?;

    if let Some(reason) = index.is_revoked(&proj.id, None) {
        return Err(format!(
            "'{}' was revoked in the Horizon Registry: {reason}",
            proj.name
        ));
    }

    let ver = proj
        .versions
        .iter()
        .find(|v| index.is_revoked(&proj.id, Some(&v.file.sha256)).is_none())
        .ok_or_else(|| format!("'{}' has no non-revoked releases available.", proj.name))?;

    if !allowed_registry_download_url(&ver.file.url) {
        return Err(format!(
            "Refusing registry download from untrusted host: {}",
            ver.file.url
        ));
    }

    let bytes = download_bytes(&ver.file.url, Some(ver.file.size.max(1) + 4096))?;
    if ver.file.size > 0 && bytes.len() as u64 != ver.file.size {
        return Err(format!(
            "Size mismatch for '{}': expected {} B, got {} B.",
            ver.file.name,
            ver.file.size,
            bytes.len()
        ));
    }

    let actual_sha = hex_encode(&Sha256::digest(&bytes));
    if !actual_sha.eq_ignore_ascii_case(ver.file.sha256.trim()) {
        return Err(format!(
            "SHA-256 verification failed for '{}'.",
            ver.file.name
        ));
    }

    // Verify author's minisign signature whenever the registry pins a public key!
    if let Some(pub_key_b64) = proj.key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        let sig_txt = ver.file.signature.as_deref().ok_or_else(|| {
            format!(
                "'{}' requires a minisign signature, but version {} has none.",
                proj.name, ver.version
            )
        })?;
        verify_minisign(&bytes, pub_key_b64, sig_txt)?;
    }

    // Stage to temp file and install according to kind.
    let stage_dir = paths::cache_dir().join("mod-stage");
    fs::create_dir_all(&stage_dir).map_err(|e| e.to_string())?;
    let safe_leaf = ver
        .file
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("download.hmod");
    let staged_file = stage_dir.join(safe_leaf);
    fs::write(&staged_file, &bytes).map_err(|e| e.to_string())?;

    let result = match proj.kind.as_str() {
        "resourcepack" => {
            let rp_dir = home.join("resourcepacks");
            fs::create_dir_all(&rp_dir).map_err(|e| e.to_string())?;
            let dst = rp_dir.join(safe_leaf);
            fs::copy(&staged_file, &dst).map_err(|e| e.to_string())?;
            Ok(format!("Installed resource pack {} v{}", proj.name, ver.version))
        }
        "shader" | "shaderpack" => {
            let sp_dir = home.join("shaderpacks");
            fs::create_dir_all(&sp_dir).map_err(|e| e.to_string())?;
            let dst = sp_dir.join(safe_leaf);
            fs::copy(&staged_file, &dst).map_err(|e| e.to_string())?;
            Ok(format!("Installed shader {} v{}", proj.name, ver.version))
        }
        "native-mod" => {
            // Ensure native-mod always routes to `horizon-native-mods/` even if manifest omits `[native]`.
            let inspected = inspect_hmod_archive(&staged_file)?;
            let native_dir = home.join("horizon-native-mods");
            fs::create_dir_all(&native_dir).map_err(|e| e.to_string())?;
            remove_existing_copies_by_id(home, &inspected.id);
            let ext = if safe_leaf.to_ascii_lowercase().ends_with(".hnmod") {
                "hnmod"
            } else {
                "hmod"
            };
            let dst = native_dir.join(format!("{}.{ext}", inspected.id));
            fs::copy(&staged_file, &dst).map_err(|e| e.to_string())?;
            set_mod_state(home, &inspected.id, true)?;
            Ok(format!(
                "Installed {} v{} (Native SDK — links automatically on Play)",
                proj.name, ver.version
            ))
        }
        _ => {
            let inspected = install_local_horizon_mod(home, &staged_file)?;
            Ok(format!(
                "Installed {} v{} ({})",
                inspected.name,
                inspected.version,
                inspected.runtime.badge()
            ))
        }
    };

    let _ = fs::remove_file(&staged_file);
    result
}

fn verify_minisign(bytes: &[u8], pub_key_base64: &str, sig_text: &str) -> Result<(), String> {
    let pk = minisign_verify::PublicKey::from_base64(pub_key_base64.trim())
        .map_err(|e| format!("Invalid registry public key: {e}"))?;
    let sig = minisign_verify::Signature::decode(sig_text)
        .map_err(|e| format!("Invalid minisign signature: {e}"))?;
    pk.verify(bytes, &sig, false)
        .map_err(|e| format!("Minisign signature verification failed: {e}"))
}

// ---------------------------------------------------------------------------
// Install Horizon Mod from GitHub URL (`owner/repo`, release tag, or `.hmod` URL)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GhRepoRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhRepoAsset>,
}

#[derive(Deserialize)]
struct GhRepoAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Parses a GitHub reference (`"owner/repo"`, `"https://github.com/owner/repo"`,
/// `"https://github.com/owner/repo/releases/tag/v0.2.0"`, or a direct `.hmod` release asset URL)
/// and installs the Horizon mod into `home`.
pub fn install_from_github(home: &Path, input: &str) -> Result<InspectedHorizonMod, String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Enter a GitHub repository (owner/repo or https://github.com/owner/repo).".into());
    }

    let stage_dir = paths::cache_dir().join("github-mod-stage");
    fs::create_dir_all(&stage_dir).map_err(|e| e.to_string())?;

    // Direct `.hmod` / `.hnmod` HTTPS download link on GitHub.
    let lower = trimmed.to_ascii_lowercase();
    if allowed_registry_download_url(trimmed)
        && (lower.ends_with(".hmod") || lower.ends_with(".hnmod"))
    {
        let bytes = download_bytes(trimmed, Some(128 << 20))?;
        let leaf = trimmed.rsplit('/').next().unwrap_or("github.hmod");
        let staged = stage_dir.join(leaf);
        fs::write(&staged, &bytes).map_err(|e| e.to_string())?;
        let res = install_local_horizon_mod(home, &staged);
        let _ = fs::remove_file(&staged);
        return res;
    }

    let (owner, repo, tag) = parse_github_spec(trimmed)?;
    let api_url = match tag {
        Some(ref t) => format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{t}"),
        None => format!("https://api.github.com/repos/{owner}/{repo}/releases/latest"),
    };

    let agent = http_agent(25);
    if let Ok(resp) = agent
        .get(&api_url)
        .set("User-Agent", &format!("FNH/{}", crate::VERSION))
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        if let Ok(rel) = resp.into_json::<GhRepoRelease>() {
            if let Some(asset) = rel.assets.iter().find(|a| {
                let n = a.name.to_ascii_lowercase();
                n.ends_with(".hmod") || n.ends_with(".hnmod")
            }) {
                if !allowed_registry_download_url(&asset.browser_download_url) {
                    return Err("Untrusted release asset URL.".into());
                }
                let bytes =
                    download_bytes(&asset.browser_download_url, Some(asset.size.max(128 << 20)))?;
                let staged = stage_dir.join(&asset.name);
                fs::write(&staged, &bytes).map_err(|e| e.to_string())?;
                let res = install_local_horizon_mod(home, &staged);
                let _ = fs::remove_file(&staged);
                if let Ok(mut ok) = res {
                    if ok.version == "0.1.0" && !rel.tag_name.is_empty() {
                        ok.version = rel.tag_name.trim_start_matches('v').to_owned();
                    }
                    return Ok(ok);
                } else {
                    return res;
                }
            }
        }
    }

    // Fallback: source repository containing `horizon-mod.toml` at root -> download zipball & pack `.hmod`.
    let zipball_url = match tag {
        Some(ref t) => format!("https://codeload.github.com/{owner}/{repo}/zip/refs/tags/{t}"),
        None => format!("https://api.github.com/repos/{owner}/{repo}/zipball"),
    };
    let zip_bytes = download_bytes(&zipball_url, Some(128 << 20)).map_err(|e| {
        format!(
            "No .hmod release asset found on {owner}/{repo}, and source archive download failed: {e}"
        )
    })?;
    let repacked = stage_dir.join(format!("{}.hmod", sanitize_mod_id(&repo)));
    repack_github_zipball_to_hmod(&zip_bytes, &repacked)?;
    let res = install_local_horizon_mod(home, &repacked);
    let _ = fs::remove_file(&repacked);
    res
}

fn parse_github_spec(input: &str) -> Result<(String, String, Option<String>), String> {
    let stripped = input
        .strip_prefix("https://github.com/")
        .or_else(|| input.strip_prefix("http://github.com/"))
        .or_else(|| input.strip_prefix("github.com/"))
        .unwrap_or(input);
    let parts: Vec<&str> = stripped
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return Err(
            "Expected GitHub repository in the form 'owner/repo' or 'https://github.com/owner/repo'."
                .into(),
        );
    }
    let owner = parts[0].to_owned();
    let repo = parts[1].trim_end_matches(".git").to_owned();
    let tag = if parts.len() >= 5 && parts[2] == "releases" && parts[3] == "tag" {
        Some(parts[4].to_owned())
    } else {
        None
    };
    Ok((owner, repo, tag))
}

fn repack_github_zipball_to_hmod(zipball: &[u8], out_hmod: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(zipball);
    let mut src =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid GitHub source archive: {e}"))?;

    let out_file = fs::File::create(out_hmod).map_err(|e| e.to_string())?;
    let mut dst = zip::ZipWriter::new(out_file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut saw_manifest = false;
    for i in 0..src.len() {
        let mut entry = src.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let full = entry
            .name()
            .map_err(|e| e.to_string())?
            .replace('\\', "/");
        // Strip GitHub's single top-level `<owner>-<repo>-<sha>/` folder prefix.
        let Some((_, rel)) = full.split_once('/') else {
            continue;
        };
        if rel.is_empty() || rel.contains("..") || rel.starts_with(".git/") {
            continue;
        }
        if rel == "horizon-mod.toml" {
            saw_manifest = true;
        }
        dst.start_file(rel, opts).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut dst).map_err(|e| e.to_string())?;
    }
    dst.finish().map_err(|e| e.to_string())?;
    if !saw_manifest {
        let _ = fs::remove_file(out_hmod);
        return Err("Repository does not contain a horizon-mod.toml manifest at its root.".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Official Upstream Native Mod SDK Bridge (`horizon-launcher native-build`)
// ---------------------------------------------------------------------------

/// Structured error returned when Native Mod SDK linking fails before launching Horizon.
#[derive(Clone, Debug)]
pub struct NativeBuildFailure {
    pub summary: String,
    pub log_output: String,
    pub enabled_native_mods: Vec<(String, String, PathBuf)>,
}

/// Lists all native mods in `<home>/horizon-native-mods/` that are currently enabled according to
/// `<home>/horizon-mods.txt` (mods default to enabled when not explicitly set to `false`).
pub fn enabled_native_mods(home: &Path) -> Vec<(String, String, PathBuf)> {
    let native_dir = home.join("horizon-native-mods");
    let Ok(rd) = fs::read_dir(&native_dir) else {
        return Vec::new();
    };
    let state = read_mods_state(home);
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        let name_str = entry.file_name().to_string_lossy().to_string();
        let valid_entry = p.is_dir()
            || name_str.to_ascii_lowercase().ends_with(".hmod")
            || name_str.to_ascii_lowercase().ends_with(".hnmod");
        if !valid_entry {
            continue;
        }
        if let Ok(info) = inspect_any_mod_path(&p, true) {
            let is_on = state.get(&info.id).copied().unwrap_or(true);
            if is_on {
                out.push((info.id, info.name, p));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Synchronizes native Horizon mods before launching `instance_id`:
/// - When **no** native mods are enabled, clears any stale `native-...` pinned version on the
///   instance so disabling/removing the last native mod cleanly launches standard Horizon!
/// - When **one or more** native mods are enabled, ensures the official `horizon-launcher` CLI
///   (`launcher@*` release from `horizon-rust/horizon`, verified against `SHA256SUMS`) is cached in
///   `<FnH>/tools/horizon-launcher/` and invokes
///   `horizon-launcher native-build --instance <instance_id> --trust-native-code`.
pub fn sync_native_mods_before_launch<F>(
    instance_id: &str,
    home: &Path,
    mut on_log: F,
) -> Result<(), NativeBuildFailure>
where
    F: FnMut(&str),
{
    let active_native = enabled_native_mods(home);
    if active_native.is_empty() {
        clear_stale_native_version_pin(instance_id, &mut on_log);
        return Ok(());
    }

    let names: Vec<&str> = active_native.iter().map(|(_, n, _)| n.as_str()).collect();
    on_log(&format!(
        "Linking {} enabled Horizon Native SDK mod(s): {}…",
        active_native.len(),
        names.join(", ")
    ));

    let cli_exe = ensure_horizon_native_builder(&mut on_log).map_err(|e| NativeBuildFailure {
        summary: e.clone(),
        log_output: e,
        enabled_native_mods: active_native.clone(),
    })?;

    let mut cmd = Command::new(&cli_exe);
    cmd.arg("native-build")
        .arg("--instance")
        .arg(instance_id)
        .arg("--trust-native-code")
        .env("HORIZON_LAUNCHER_HOME", paths::launcher_dir())
        .env("HORIZON_HOME", paths::game_root())
        .env("HORIZON_BUILD_DIR", paths::root().join("build"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        let msg = format!("Could not start Horizon Native SDK builder ({}): {e}", cli_exe.display());
        NativeBuildFailure {
            summary: msg.clone(),
            log_output: msg,
            enabled_native_mods: active_native.clone(),
        }
    })?;

    let collected = Arc::new(Mutex::new(Vec::<String>::new()));
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();

    let c1 = collected.clone();
    let t_out = std::thread::spawn(move || {
        if let Some(pipe) = out_pipe {
            use std::io::{BufRead, BufReader};
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                if let Ok(mut g) = c1.lock() {
                    g.push(line);
                }
            }
        }
    });
    let c2 = collected.clone();
    let t_err = std::thread::spawn(move || {
        if let Some(pipe) = err_pipe {
            use std::io::{BufRead, BufReader};
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                if let Ok(mut g) = c2.lock() {
                    g.push(line);
                }
            }
        }
    });

    let mut printed = 0usize;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) => {
                if let Ok(g) = collected.lock() {
                    while printed < g.len() {
                        on_log(&format!("[native-sdk] {}", g[printed]));
                        printed += 1;
                    }
                }
                std::thread::sleep(Duration::from_millis(80));
            }
            Err(e) => break Err(e),
        }
    };

    let _ = t_out.join();
    let _ = t_err.join();

    let all_lines = collected
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    while printed < all_lines.len() {
        on_log(&format!("[native-sdk] {}", all_lines[printed]));
        printed += 1;
    }

    match status {
        Ok(st) if st.success() => {
            on_log("Horizon Native Mod SDK build finished cleanly.");
            Ok(())
        }
        Ok(st) => {
            let full_log = all_lines.join("\n");
            let code = st.code().unwrap_or(-1);
            let summary = summarize_native_build_failure(code, &full_log);
            Err(NativeBuildFailure {
                summary,
                log_output: full_log,
                enabled_native_mods: active_native,
            })
        }
        Err(e) => Err(NativeBuildFailure {
            summary: format!("Failed waiting on Horizon Native Mod SDK build: {e}"),
            log_output: all_lines.join("\n"),
            enabled_native_mods: active_native,
        }),
    }
}

fn summarize_native_build_failure(exit_code: i32, log_output: &str) -> String {
    let lower = log_output.to_ascii_lowercase();
    if lower.contains("cargo") && (lower.contains("not found") || lower.contains("install rust")) {
        "Native Rust mod requires the Rust toolchain (`cargo` / `rustup`) installed on Windows."
            .into()
    } else if lower.contains("sdk-") && lower.contains("not") {
        "The installed Horizon client build does not match a published Native Mod SDK slice yet."
            .into()
    } else {
        all_nonempty_last_error(log_output).unwrap_or_else(|| {
            format!("Horizon Native Mod SDK build exited with code {exit_code}.")
        })
    }
}

fn all_nonempty_last_error(log_output: &str) -> Option<String> {
    log_output
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.to_owned())
}

fn clear_stale_native_version_pin<F: FnMut(&str)>(instance_id: &str, on_log: &mut F) {
    if let Ok(inst) = horizon_launcher::instances::find(instance_id) {
        if inst
            .version
            .as_deref()
            .is_some_and(|v| v.starts_with("native-"))
        {
            on_log(
                "No native mods enabled — switching Horizon instance back to standard client build.",
            );
            let _ = horizon_launcher::instances::edit(instance_id, |m| {
                m.version = None;
            });
        }
    }
}

#[derive(Deserialize)]
struct UpstreamLauncherRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhRepoAsset>,
}

fn ensure_horizon_native_builder<F: FnMut(&str)>(on_log: &mut F) -> Result<PathBuf, String> {
    let tool_dir = paths::root().join("tools").join("horizon-launcher");
    let exe_name = if cfg!(windows) {
        "horizon-launcher.exe"
    } else {
        "horizon-launcher"
    };
    let exe_path = tool_dir.join(exe_name);
    let stamp_path = tool_dir.join("release-tag.txt");

    let asset_name = if cfg!(windows) {
        "horizon-launcher-windows-x64.zip"
    } else if cfg!(target_os = "macos") {
        "horizon-launcher-macos-arm64.tar.gz"
    } else {
        "horizon-launcher-linux-x64.tar.gz"
    };

    let agent = http_agent(25);
    let releases: Vec<UpstreamLauncherRelease> = match agent
        .get(HORIZON_RELEASES_API)
        .set("User-Agent", &format!("FNH/{}", crate::VERSION))
        .set("Accept", "application/vnd.github+json")
        .call()
        .and_then(|r| r.into_json().map_err(ureq::Error::from))
    {
        Ok(list) => list,
        Err(net_err) => {
            if exe_path.is_file() {
                return Ok(exe_path);
            }
            return Err(format!(
                "Could not fetch Horizon Native SDK builder release from GitHub: {net_err}"
            ));
        }
    };

    let Some(rel) = releases.into_iter().find(|r| {
        !r.draft
            && r.tag_name.starts_with("launcher@")
            && r.assets.iter().any(|a| a.name == asset_name)
    }) else {
        if exe_path.is_file() {
            return Ok(exe_path);
        }
        return Err("No official `launcher@*` Native SDK builder release found on horizon-rust/horizon.".into());
    };

    if exe_path.is_file()
        && fs::read_to_string(&stamp_path)
            .ok()
            .as_deref()
            .map(str::trim)
            == Some(rel.tag_name.as_str())
    {
        return Ok(exe_path);
    }

    on_log(&format!(
        "Downloading official Horizon Native SDK builder ({})…",
        rel.tag_name
    ));
    fs::create_dir_all(&tool_dir).map_err(|e| e.to_string())?;

    let zip_asset = rel
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("Missing {asset_name} in {}", rel.tag_name))?;

    let archive_bytes =
        download_bytes(&zip_asset.browser_download_url, Some(256 << 20))?;

    // Verify SHA-256 against `SHA256SUMS` published on the release!
    if let Some(sums_asset) = rel.assets.iter().find(|a| a.name == "SHA256SUMS") {
        let sums_bytes = download_bytes(&sums_asset.browser_download_url, Some(1 << 20))?;
        let sums_text = String::from_utf8_lossy(&sums_bytes);
        if let Some(expected) = parse_sha256sums_entry(&sums_text, asset_name) {
            let actual = hex_encode(&Sha256::digest(&archive_bytes));
            if !actual.eq_ignore_ascii_case(&expected) {
                return Err(format!(
                    "SHA-256 verification failed for {asset_name} in {}.",
                    rel.tag_name
                ));
            }
        }
    }

    if asset_name.ends_with(".zip") {
        let cursor = std::io::Cursor::new(&archive_bytes);
        let mut zip = zip::ZipArchive::new(cursor)
            .map_err(|e| format!("Invalid {asset_name}: {e}"))?;
        let mut extracted = false;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let leaf = entry
                .name()
                .map_err(|e| e.to_string())?
                .replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_owned();
            if leaf.eq_ignore_ascii_case(exe_name) {
                let mut out = fs::File::create(&exe_path).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
                out.flush().map_err(|e| e.to_string())?;
                extracted = true;
                break;
            }
        }
        if !extracted {
            return Err(format!("{asset_name} did not contain {exe_name}."));
        }
    } else {
        return Err("Native SDK automatic extraction is configured for Windows x64 builds.".into());
    }

    let _ = fs::write(&stamp_path, &rel.tag_name);
    Ok(exe_path)
}

fn parse_sha256sums_entry(text: &str, filename: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if name.rsplit('/').next() == Some(filename) && hash.len() == 64 {
            return Some(hash.to_owned());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// HTTP & Hex Helpers
// ---------------------------------------------------------------------------

fn http_agent(read_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(12))
        .timeout_read(Duration::from_secs(read_secs))
        .build()
}

fn download_bytes(url: &str, max_bytes: Option<u64>) -> Result<Vec<u8>, String> {
    let cap = max_bytes.unwrap_or(128 << 20);
    let resp = http_agent(120)
        .get(url)
        .set("User-Agent", &format!("FNH/{}", crate::VERSION))
        .call()
        .map_err(|e| format!("Download failed ({url}): {e}"))?;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 32 * 1024];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("Network read error: {e}"))?;
        if n == 0 {
            break;
        }
        if (buf.len() + n) as u64 > cap {
            return Err(format!("Download exceeded safety size limit ({cap} bytes)."));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(buf)
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_content_wasm_and_native_hmods_and_routes_correctly() {
        let tmp = std::env::temp_dir().join(format!("fnh-hmod-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let home = tmp.join("inst");
        fs::create_dir_all(&home).unwrap();

        let opts = zip::write::SimpleFileOptions::default();

        // 1. WASM `.hmod` -> `<home>/horizon-mods/zoom_hud.hmod` + `mod.zoom_hud:true`
        let wasm_bytes = b"\0asm\x01\x00\x00\x00";
        let sha512 = hex_encode(&Sha512::digest(wasm_bytes));
        let wasm_zip = tmp.join("zoom-download.hmod");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&wasm_zip).unwrap());
            z.start_file("horizon-mod.toml", opts).unwrap();
            write!(
                z,
                "id = \"zoom_hud\"\nname = \"Zoom HUD\"\nversion = \"0.3.0\"\napi = \"0.2\"\n\n[wasm]\nfile = \"mod.wasm\"\nsha512 = \"{sha512}\"\n"
            )
            .unwrap();
            z.start_file("mod.wasm", opts).unwrap();
            z.write_all(wasm_bytes).unwrap();
            z.finish().unwrap();
        }

        let installed_wasm = install_local_horizon_mod(&home, &wasm_zip).unwrap();
        assert_eq!(installed_wasm.id, "zoom_hud");
        assert_eq!(installed_wasm.runtime, HorizonModRuntime::Wasm);
        assert!(home.join("horizon-mods/zoom_hud.hmod").is_file());
        assert_eq!(read_mods_state(&home).get("zoom_hud"), Some(&true));

        // 2. Native `.hmod` (`[native]` + `Cargo.toml`) -> `<home>/horizon-native-mods/sodium_native.hmod`
        let native_zip = tmp.join("sodium.hmod");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&native_zip).unwrap());
            z.start_file("horizon-mod.toml", opts).unwrap();
            write!(
                z,
                "id = \"sodium_native\"\nname = \"Sodium Native\"\nversion = \"0.2.1\"\napi = \"0.2\"\n\n[native]\ncrate = \"sodium_native\"\n"
            )
            .unwrap();
            z.start_file("Cargo.toml", opts).unwrap();
            write!(z, "[package]\nname = \"sodium_native\"\nversion = \"0.2.1\"\n").unwrap();
            z.finish().unwrap();
        }

        let installed_native = install_local_horizon_mod(&home, &native_zip).unwrap();
        assert_eq!(installed_native.id, "sodium_native");
        assert_eq!(installed_native.runtime, HorizonModRuntime::Native);
        assert!(home.join("horizon-native-mods/sodium_native.hmod").is_file());
        assert_eq!(enabled_native_mods(&home).len(), 1);

        // Toggling `mod.sodium_native:false` disables it from `enabled_native_mods`.
        set_mod_state(&home, "sodium_native", false).unwrap();
        assert!(enabled_native_mods(&home).is_empty());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parses_v02_horizon_registry_index_with_native_hmod() {
        let json = r#"{
            "api": "0.2",
            "projects": [
                {
                    "id": "trinkets",
                    "kind": "native-mod",
                    "name": "Trinkets",
                    "summary": "Accessory slots for Horizon",
                    "authors": ["Buffer"],
                    "downloads": 420,
                    "versions": [
                        {
                            "version": "0.2.1",
                            "api": "0.2",
                            "game": "1.21.11",
                            "native": "native-1/windows-x64",
                            "file": {
                                "name": "trinkets-0.2.1.hmod",
                                "url": "https://github.com/horizon-rust/trinkets/releases/download/v0.2.1/trinkets-0.2.1.hmod",
                                "size": 12345,
                                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                            }
                        }
                    ]
                }
            ],
            "revoked": []
        }"#;
        let idx = parse_registry_index(json).unwrap();
        assert_eq!(idx.api, "0.2");
        assert_eq!(idx.projects.len(), 1);
        assert_eq!(idx.projects[0].kind, "native-mod");
        assert!(idx.projects[0].versions[0].file.name.ends_with(".hmod"));
    }
}
