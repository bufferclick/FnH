//! Fabric-Loader-style Crash & Unsupported Mod Analyzer (`v1.7`).
//!
//! Covers both **pre-launch** mod compatibility checks (Forge/NeoForge `.jar` dropped into Fabric,
//! missing `fabric-api` / mod dependencies with full `META-INF/jars/*.jar` Jar-in-Jar awareness,
//! raw Java `.jar` dropped into Horizon without porting, missing Horizon `.hmod` dependencies) and
//! **post-exit** crash / `FormattedException` / `Incompatible mod set!` / `crash-reports/crash-*.txt`
//! / Horizon Native Mod SDK failure diagnostics — powering FNH's dedicated `CrashWindow`.

use crate::hmods;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// High-level category shown in the `CrashWindow` header banner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashKind {
    /// Fabric Loader `Incompatible mod set!`, missing dependency, duplicate mod ID, or wrong loader.
    IncompatibleModSet,
    /// Forge/NeoForge `.jar` in Fabric, raw Java `.jar` in Horizon, or Native SDK incompatibility.
    UnsupportedMod,
    /// Runtime crash (`Minecraft Crash Report`, Mixin error, JVM/Horizon non-zero exit or panic).
    GameCrash,
}

impl CrashKind {
    pub fn badge(self) -> &'static str {
        match self {
            Self::IncompatibleModSet => "INCOMPATIBLE MOD SET",
            Self::UnsupportedMod => "UNSUPPORTED MOD",
            Self::GameCrash => "GAME CRASH",
        }
    }
}

/// An offending or suspect mod surfaced in the crash dialog, with optional path for 1-click Disable.
#[derive(Clone, Debug)]
pub struct SuspectMod {
    pub mod_id: String,
    pub display_name: String,
    pub reason: String,
    pub path: Option<PathBuf>,
    pub is_horizon: bool,
    pub disabled: bool,
}

/// Complete structured report rendered by `CrashWindow`.
#[derive(Clone, Debug)]
pub struct CrashDiagnosis {
    pub instance_id: String,
    pub instance_name: String,
    pub engine_label: String,
    pub is_horizon: bool,
    pub kind: CrashKind,
    pub headline: String,
    pub summary: String,
    pub solutions: Vec<String>,
    pub offending_mods: Vec<SuspectMod>,
    pub raw_details: String,
    pub crash_file: Option<PathBuf>,
    pub mods_dir: PathBuf,
}

static PENDING_CRASHES: OnceLock<Mutex<Vec<CrashDiagnosis>>> = OnceLock::new();

fn queue() -> &'static Mutex<Vec<CrashDiagnosis>> {
    PENDING_CRASHES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Pushes a newly detected crash / incompatible-mod report so the UI thread opens `CrashWindow`.
pub fn push_diagnosis(diag: CrashDiagnosis) {
    if let Ok(mut guard) = queue().lock() {
        guard.push(diag);
    }
}

/// Drains any pending crash / mod-error reports on the UI thread.
pub fn take_pending_diagnoses() -> Vec<CrashDiagnosis> {
    queue()
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}

/// Disables a `SuspectMod` in-place (renames `.jar` -> `.jar.disabled`, or sets `mod.<id>:false`
/// in `horizon-mods.txt`).
pub fn disable_suspect_mod(home: &Path, m: &SuspectMod) -> Result<String, String> {
    if m.is_horizon {
        if let Some(ref p) = m.path {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext == "jar" {
                let mut new_os = p.as_os_str().to_os_string();
                new_os.push(".disabled");
                let dst = PathBuf::from(new_os);
                fs::rename(p, &dst).map_err(|e| e.to_string())?;
                return Ok(format!("Disabled unsupported Java jar '{}'.", m.display_name));
            }
        }
        hmods::set_mod_state(home, &m.mod_id, false)?;
        Ok(format!("Disabled Horizon mod '{}'.", m.display_name))
    } else if let Some(ref p) = m.path {
        if !p.exists() {
            return Ok(format!("'{}' is already disabled or removed.", m.display_name));
        }
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("mod.jar");
        if name.ends_with(".disabled") {
            return Ok(format!("'{}' is already disabled.", m.display_name));
        }
        let dst = p.with_file_name(format!("{name}.disabled"));
        fs::rename(p, &dst).map_err(|e| format!("Could not disable {}: {e}", name))?;
        Ok(format!("Disabled '{}'.", m.display_name))
    } else {
        Err("Mod file path is not available to disable automatically.".into())
    }
}

// ---------------------------------------------------------------------------
// Pre-Launch Mod Compatibility Scanner (Fabric/Java & Horizon)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct FabricModJson {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: serde_json::Value,
    #[serde(default)]
    provides: Vec<String>,
    #[serde(default)]
    depends: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    breaks: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    jars: Vec<NestedJarEntry>,
}

#[derive(Deserialize, Default)]
struct NestedJarEntry {
    #[serde(default)]
    file: String,
}

#[derive(Clone, Debug)]
struct ScannedJavaJar {
    path: PathBuf,
    filename: String,
    id: String,
    name: String,
    version: String,
    provides: Vec<String>,
    depends: BTreeMap<String, String>,
    breaks: BTreeMap<String, String>,
    wrong_loader: Option<&'static str>,
}

/// Scans an instance's mods *before* spawning the game so blatantly unsupported mods (e.g. a
/// Forge/NeoForge-only `.jar` dropped into Fabric, `.jar` mods in a Vanilla no-loader instance,
/// missing `fabric-api`, or unported raw `.jar` files dropped into a Horizon instance) immediately
/// surface in the Fabric-style compatibility dialog with 1-click fixes!
pub fn check_prelaunch_compatibility(
    instance_id: &str,
    instance_name: &str,
    home: &Path,
    is_horizon: bool,
    mc_version: &str,
    fabric_enabled: bool,
) -> Option<CrashDiagnosis> {
    if is_horizon {
        check_horizon_prelaunch(instance_id, instance_name, home)
    } else {
        check_java_prelaunch(
            instance_id,
            instance_name,
            home,
            mc_version,
            fabric_enabled,
        )
    }
}

fn check_java_prelaunch(
    instance_id: &str,
    instance_name: &str,
    home: &Path,
    mc_version: &str,
    fabric_enabled: bool,
) -> Option<CrashDiagnosis> {
    let mods_dir = home.join("mods");
    let Ok(rd) = fs::read_dir(&mods_dir) else {
        return None;
    };

    let mut jars = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !p.is_file() || !fname.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        if let Some(scanned) = inspect_java_jar(&p, fname) {
            jars.push(scanned);
        }
    }

    if jars.is_empty() {
        return None;
    }

    let engine_label = if fabric_enabled {
        format!("Fabric {mc_version}")
    } else {
        format!("Minecraft {mc_version} (Vanilla)")
    };

    // 1. Mods placed in `mods/` while Fabric Loader is turned OFF!
    if !fabric_enabled {
        let suspects: Vec<SuspectMod> = jars
            .iter()
            .map(|j| SuspectMod {
                mod_id: j.id.clone(),
                display_name: format!("{} ({})", j.name, j.filename),
                reason: "Fabric Loader is disabled on this Vanilla instance.".into(),
                path: Some(j.path.clone()),
                is_horizon: false,
                disabled: false,
            })
            .collect();
        return Some(CrashDiagnosis {
            instance_id: instance_id.to_owned(),
            instance_name: instance_name.to_owned(),
            engine_label,
            is_horizon: false,
            kind: CrashKind::UnsupportedMod,
            headline: "Mods Installed Without a Mod Loader".into(),
            summary: format!(
                "Found {} mod jar(s) in mods/, but '{}' is configured as Vanilla Minecraft without Fabric Loader.",
                jars.len(),
                instance_name
            ),
            solutions: vec![
                "Open Instance -> Version and enable Fabric Loader so your Fabric mods can load.".into(),
                "Or disable the mod jar(s) below to launch pure Vanilla Minecraft.".into(),
            ],
            offending_mods: suspects,
            raw_details: jars
                .iter()
                .map(|j| format!("- {} ({})", j.filename, j.id))
                .collect::<Vec<_>>()
                .join("\n"),
            crash_file: None,
            mods_dir,
        });
    }

    // 2. Forge / NeoForge-only `.jar` placed in a Fabric instance!
    let mut offending = Vec::new();
    let mut solutions = Vec::new();
    let mut raw_lines = Vec::new();

    for j in &jars {
        if let Some(other_loader) = j.wrong_loader {
            offending.push(SuspectMod {
                mod_id: j.id.clone(),
                display_name: format!("{} ({})", j.name, j.filename),
                reason: format!("{other_loader} mod — incompatible with Fabric Loader."),
                path: Some(j.path.clone()),
                is_horizon: false,
                disabled: false,
            });
            raw_lines.push(format!(
                "Unsupported loader: '{}' is packaged for {other_loader} (missing fabric.mod.json).",
                j.filename
            ));
        }
    }
    if !offending.is_empty() {
        solutions.push(
            "Replace the Forge/NeoForge mod(s) below with their Fabric edition from Browse Mods (Modrinth)."
                .into(),
        );
        solutions.push("Or click 'Disable' next to the unsupported mod(s) below and relaunch.".into());
        return Some(CrashDiagnosis {
            instance_id: instance_id.to_owned(),
            instance_name: instance_name.to_owned(),
            engine_label,
            is_horizon: false,
            kind: CrashKind::UnsupportedMod,
            headline: "Unsupported Forge / NeoForge Mod Detected!".into(),
            summary:
                "One or more .jar files in mods/ were built for Forge or NeoForge and cannot run on Fabric Loader."
                    .into(),
            solutions,
            offending_mods: offending,
            raw_details: raw_lines.join("\n"),
            crash_file: None,
            mods_dir,
        });
    }

    // 3. Collect all provided Fabric mod IDs (including nested Jar-in-Jar `META-INF/jars/*.jar`!).
    let mut provided_ids: BTreeSet<String> = [
        "minecraft",
        "java",
        "fabricloader",
        "mixinextras",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    let mut seen_top_ids: BTreeMap<String, &ScannedJavaJar> = BTreeMap::new();
    for j in &jars {
        if let Some(prev) = seen_top_ids.insert(j.id.clone(), j) {
            offending.push(SuspectMod {
                mod_id: j.id.clone(),
                display_name: format!("{} ({})", j.name, j.filename),
                reason: format!("Duplicate of '{}' ({})", prev.name, prev.filename),
                path: Some(j.path.clone()),
                is_horizon: false,
                disabled: false,
            });
            raw_lines.push(format!(
                "Duplicate mod ID '{}': '{}' and '{}'",
                j.id, prev.filename, j.filename
            ));
        }
        provided_ids.insert(j.id.clone());
        for p in &j.provides {
            provided_ids.insert(p.clone());
        }
    }

    // Check missing required dependencies & explicit `breaks`.
    for j in &jars {
        for (dep_id, req) in &j.depends {
            if !provided_ids.contains(dep_id) {
                let friendly_dep = if dep_id == "fabric-api" || dep_id == "fabric" {
                    "Fabric API (fabric-api)"
                } else {
                    dep_id.as_str()
                };
                let sol = format!(
                    "Install '{}' ({}) via Mods -> Browse Mods, or disable '{}'.",
                    friendly_dep, req, j.name
                );
                if !solutions.contains(&sol) {
                    solutions.push(sol);
                }
                offending.push(SuspectMod {
                    mod_id: j.id.clone(),
                    display_name: format!("{} v{}", j.name, j.version),
                    reason: format!("Requires missing mod '{friendly_dep}' ({req})"),
                    path: Some(j.path.clone()),
                    is_horizon: false,
                    disabled: false,
                });
                raw_lines.push(format!(
                    "Mod '{}' ({}) {} requires '{}' ({}), which is missing!",
                    j.name, j.id, j.version, dep_id, req
                ));
            }
        }
        for (brk_id, req) in &j.breaks {
            if req == "*" && provided_ids.contains(brk_id) {
                offending.push(SuspectMod {
                    mod_id: j.id.clone(),
                    display_name: format!("{} v{}", j.name, j.version),
                    reason: format!("Conflicts with installed mod '{brk_id}'"),
                    path: Some(j.path.clone()),
                    is_horizon: false,
                    disabled: false,
                });
                raw_lines.push(format!(
                    "Mod '{}' ({}) declares `breaks` against installed mod '{}'.",
                    j.name, j.id, brk_id
                ));
            }
        }
    }

    if offending.is_empty() {
        return None;
    }

    if solutions.is_empty() {
        solutions.push("Disable one of the conflicting/duplicate mods below and relaunch.".into());
    }

    Some(CrashDiagnosis {
        instance_id: instance_id.to_owned(),
        instance_name: instance_name.to_owned(),
        engine_label,
        is_horizon: false,
        kind: CrashKind::IncompatibleModSet,
        headline: "Incompatible mod set!".into(),
        summary:
            "Fabric mod dependency check found missing required libraries or conflicting mods before launch."
                .into(),
        solutions,
        offending_mods: offending,
        raw_details: raw_lines.join("\n"),
        crash_file: None,
        mods_dir,
    })
}

fn inspect_java_jar(path: &Path, filename: &str) -> Option<ScannedJavaJar> {
    let file = fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;

    let mut fabric_txt = None;
    let mut has_forge_toml = false;
    let mut has_neoforge_toml = false;
    let mut has_quilt_json = false;

    if let Ok(mut entry) = zip.by_name("fabric.mod.json") {
        let mut s = String::new();
        let _ = entry.read_to_string(&mut s);
        fabric_txt = Some(s);
    }
    if zip.by_name("META-INF/neoforge.mods.toml").is_ok() {
        has_neoforge_toml = true;
    }
    if zip.by_name("META-INF/mods.toml").is_ok() || zip.by_name("mcmod.info").is_ok() {
        has_forge_toml = true;
    }
    if zip.by_name("quilt.mod.json").is_ok() {
        has_quilt_json = true;
    }

    let Some(txt) = fabric_txt else {
        let wrong_loader = if has_neoforge_toml {
            Some("NeoForge")
        } else if has_forge_toml {
            Some("Forge")
        } else if has_quilt_json {
            Some("Quilt-only")
        } else {
            None
        };
        if wrong_loader.is_some() {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(filename);
            return Some(ScannedJavaJar {
                path: path.to_path_buf(),
                filename: filename.to_owned(),
                id: stem.to_owned(),
                name: stem.to_owned(),
                version: String::new(),
                provides: Vec::new(),
                depends: BTreeMap::new(),
                breaks: BTreeMap::new(),
                wrong_loader,
            });
        }
        return None;
    };

    let parsed: FabricModJson = serde_json::from_str(&txt).ok()?;
    if parsed.id.trim().is_empty() {
        return None;
    }

    let mut provides = parsed.provides;
    // Also collect nested Jar-in-Jar (`META-INF/jars/*.jar`) IDs & `provides` so bundled libraries
    // (`fabric-api` submodules, `cloth-config`, `mixinextras`, etc.) are recognized!
    for nested in &parsed.jars {
        let rel = nested.file.trim_start_matches('/');
        if rel.is_empty() {
            continue;
        }
        if let Ok(mut inner_entry) = zip.by_name(rel) {
            let mut inner_bytes = Vec::new();
            if inner_entry.read_to_end(&mut inner_bytes).is_ok() && inner_bytes.len() < (32 << 20) {
                collect_nested_jar_ids(&inner_bytes, &mut provides, 0);
            }
        }
    }
    if parsed.id == "fabric-api" {
        provides.push("fabric".into());
    }

    let version = match &parsed.version {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let depends = parsed
        .depends
        .into_iter()
        .map(|(k, v)| (k, format_dep_value(&v)))
        .collect();
    let breaks = parsed
        .breaks
        .into_iter()
        .map(|(k, v)| (k, format_dep_value(&v)))
        .collect();

    Some(ScannedJavaJar {
        path: path.to_path_buf(),
        filename: filename.to_owned(),
        name: if parsed.name.trim().is_empty() {
            parsed.id.clone()
        } else {
            parsed.name
        },
        id: parsed.id,
        version,
        provides,
        depends,
        breaks,
        wrong_loader: None,
    })
}

fn collect_nested_jar_ids(bytes: &[u8], out: &mut Vec<String>, depth: u8) {
    if depth > 2 {
        return;
    }
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return;
    };
    let mut fmj = String::new();
    if let Ok(mut e) = z.by_name("fabric.mod.json") {
        let _ = e.read_to_string(&mut fmj);
    }
    if let Ok(parsed) = serde_json::from_str::<FabricModJson>(&fmj) {
        if !parsed.id.is_empty() {
            out.push(parsed.id.clone());
            if parsed.id == "fabric-api" {
                out.push("fabric".into());
            }
        }
        out.extend(parsed.provides);
        for nested in &parsed.jars {
            let rel = nested.file.trim_start_matches('/');
            if let Ok(mut inner) = z.by_name(rel) {
                let mut buf = Vec::new();
                if inner.read_to_end(&mut buf).is_ok() && buf.len() < (16 << 20) {
                    collect_nested_jar_ids(&buf, out, depth + 1);
                }
            }
        }
    }
}

fn format_dep_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(" || "),
        other => other.to_string(),
    }
}

fn check_horizon_prelaunch(
    instance_id: &str,
    instance_name: &str,
    home: &Path,
) -> Option<CrashDiagnosis> {
    let mods_dir = home.join("horizon-mods");
    let mut unported_jars = Vec::new();

    for sub in ["mods", "horizon-mods", "horizon-native-mods"] {
        let dir = home.join(sub);
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if p.is_file() && fname.to_ascii_lowercase().ends_with(".jar") {
                unported_jars.push((fname.to_owned(), p));
            }
        }
    }

    if !unported_jars.is_empty() {
        let offending = unported_jars
            .iter()
            .map(|(fname, p)| SuspectMod {
                mod_id: fname.clone(),
                display_name: fname.clone(),
                reason:
                    "Raw Java .jar file — Horizon runs native Rust / WASM / .hmod content, not JVM bytecode."
                        .into(),
                path: Some(p.clone()),
                is_horizon: true,
                disabled: false,
            })
            .collect();
        return Some(CrashDiagnosis {
            instance_id: instance_id.to_owned(),
            instance_name: instance_name.to_owned(),
            engine_label: "Horizon".into(),
            is_horizon: true,
            kind: CrashKind::UnsupportedMod,
            headline: "Unsupported Raw Java .jar in Horizon Instance".into(),
            summary:
                "Raw Java .jar files were placed directly in this Horizon instance's folder without porting."
                    .into(),
            solutions: vec![
                "Import Java .jar mods via Instance -> Mods -> Add File (or drag-and-drop onto FNH) so `horizon-port-pack` checks compatibility and ports assets/data into a Horizon mod.".into(),
                "For gameplay/code mods, browse the Horizon Registry (`.hmod` WASM & Native SDK mods).".into(),
                "Click 'Disable' next to the raw .jar file(s) below to launch Horizon cleanly.".into(),
            ],
            offending_mods: offending,
            raw_details: unported_jars
                .iter()
                .map(|(n, p)| format!("Unported Java jar: {} ({})", n, p.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            crash_file: None,
            mods_dir,
        });
    }

    // Check `[dependencies]` across enabled Horizon `.hmod`s / folders!
    let state = hmods::read_mods_state(home);
    let mut installed_enabled: BTreeMap<String, (hmods::InspectedHorizonMod, PathBuf)> =
        BTreeMap::new();
    for (sub, is_native) in [("horizon-mods", false), ("horizon-native-mods", true)] {
        let dir = home.join(sub);
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if let Ok(info) = hmods::inspect_any_mod_path(&p, is_native) {
                if state.get(&info.id).copied().unwrap_or(true) {
                    installed_enabled.insert(info.id.clone(), (info, p));
                }
            }
        }
    }

    let mut missing_suspects = Vec::new();
    let mut solutions = Vec::new();
    let mut raw = Vec::new();
    for (id, (info, p)) in &installed_enabled {
        for (dep_id, req) in &info.dependencies {
            if dep_id == "horizon" || dep_id == "minecraft" {
                continue;
            }
            if !installed_enabled.contains_key(dep_id) {
                solutions.push(format!(
                    "Install or enable Horizon mod '{dep_id}' ({req}), required by '{}'.",
                    info.name
                ));
                raw.push(format!(
                    "Horizon mod '{}' ({id}) requires missing/disabled mod '{dep_id}' ({req}).",
                    info.name
                ));
                missing_suspects.push(SuspectMod {
                    mod_id: id.clone(),
                    display_name: format!("{} v{}", info.name, info.version),
                    reason: format!("Requires missing/disabled Horizon mod '{dep_id}' ({req})"),
                    path: Some(p.clone()),
                    is_horizon: true,
                    disabled: false,
                });
            }
        }
    }

    if missing_suspects.is_empty() {
        None
    } else {
        Some(CrashDiagnosis {
            instance_id: instance_id.to_owned(),
            instance_name: instance_name.to_owned(),
            engine_label: "Horizon".into(),
            is_horizon: true,
            kind: CrashKind::IncompatibleModSet,
            headline: "Missing Horizon Mod Dependency!".into(),
            summary:
                "One or more enabled Horizon mods require another Horizon mod that isn't installed or enabled."
                    .into(),
            solutions,
            offending_mods: missing_suspects,
            raw_details: raw.join("\n"),
            crash_file: None,
            mods_dir,
        })
    }
}

// ---------------------------------------------------------------------------
// Native Mod SDK Build Failure -> CrashDiagnosis
// ---------------------------------------------------------------------------

pub fn diagnosis_from_native_build_failure(
    instance_id: &str,
    instance_name: &str,
    home: &Path,
    failure: &hmods::NativeBuildFailure,
) -> CrashDiagnosis {
    let offending = failure
        .enabled_native_mods
        .iter()
        .map(|(id, name, path)| SuspectMod {
            mod_id: id.clone(),
            display_name: name.clone(),
            reason: "Enabled Native SDK mod during failed native-build.".into(),
            path: Some(path.clone()),
            is_horizon: true,
            disabled: false,
        })
        .collect();

    CrashDiagnosis {
        instance_id: instance_id.to_owned(),
        instance_name: instance_name.to_owned(),
        engine_label: "Horizon (Native Mod SDK)".into(),
        is_horizon: true,
        kind: CrashKind::UnsupportedMod,
        headline: "Horizon Native Mod SDK Build Failed".into(),
        summary: failure.summary.clone(),
        solutions: vec![
            "Ensure the Rust toolchain (`rustup` / `cargo` + MSVC Build Tools) is installed when linking Native SDK mods.".into(),
            "Check that your Native SDK mod supports the installed Horizon client version, or switch between Stable and Nightly in Instance -> Version.".into(),
            "Click 'Disable' next to a Native SDK mod below to launch Horizon immediately without native compilation.".into(),
        ],
        offending_mods: offending,
        raw_details: if failure.log_output.trim().is_empty() {
            failure.summary.clone()
        } else {
            failure.log_output.clone()
        },
        crash_file: None,
        mods_dir: home.join("horizon-native-mods"),
    }
}

// ---------------------------------------------------------------------------
// Post-Exit Fabric / Java / Horizon Log & Crash-Report Analyzer
// ---------------------------------------------------------------------------

/// Inspects console output + `<home>/crash-reports/crash-*.txt` after a game process exits to
/// determine if a crash or Fabric `Incompatible mod set!` / `FormattedException` occurred.
pub fn analyze_post_exit(
    instance_id: &str,
    instance_name: &str,
    engine_label: &str,
    home: &Path,
    is_horizon: bool,
    exit_code: Option<i32>,
    log_lines: &[String],
    started_at: SystemTime,
) -> Option<CrashDiagnosis> {
    let joined = log_lines.join("\n");
    let recent_crash_report = newest_crash_report_since(&home.join("crash-reports"), started_at);
    let crash_report_text = recent_crash_report
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_default();

    let saw_fabric_incompat = joined.contains("Incompatible mod set!")
        || joined.contains("net.fabricmc.loader.impl.FormattedException");
    let saw_mixin_error = joined.contains("MixinApplyError")
        || joined.contains("MixinTransformerError")
        || joined.contains("Critical injection failure");
    let saw_mc_crash = joined.contains("---- Minecraft Crash Report ----")
        || !crash_report_text.is_empty();
    let saw_horizon_panic = is_horizon
        && (joined.contains("panicked at")
            || joined.contains("mod error:")
            || joined.contains("failed to load mod"));

    let non_zero = exit_code.is_some_and(|c| c != 0);

    if !saw_fabric_incompat
        && !saw_mixin_error
        && !saw_mc_crash
        && !saw_horizon_panic
        && !non_zero
    {
        return None;
    }

    let mods_dir = if is_horizon {
        home.join("horizon-mods")
    } else {
        home.join("mods")
    };

    if saw_fabric_incompat {
        let (summary, mut solutions, mentioned_ids) = parse_fabric_formatted_exception(log_lines);
        let offending = match_mentioned_java_mods(&mods_dir, &mentioned_ids, &joined);
        if solutions.is_empty() {
            solutions.push(
                "Install the missing dependency via Mods -> Browse Mods, or disable the incompatible mod below."
                    .into(),
            );
        }
        return Some(CrashDiagnosis {
            instance_id: instance_id.to_owned(),
            instance_name: instance_name.to_owned(),
            engine_label: engine_label.to_owned(),
            is_horizon: false,
            kind: CrashKind::IncompatibleModSet,
            headline: "Incompatible mod set!".into(),
            summary,
            solutions,
            offending_mods: offending,
            raw_details: tail_text(log_lines, 140),
            crash_file: recent_crash_report,
            mods_dir,
        });
    }

    let combined_for_scan = if crash_report_text.is_empty() {
        joined.clone()
    } else {
        format!("{crash_report_text}\n\n--- Console Output ---\n{}", tail_text(log_lines, 90))
    };

    let (headline, summary) = if !crash_report_text.is_empty() {
        let desc = crash_report_text
            .lines()
            .find_map(|l| l.strip_prefix("Description:"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Minecraft generated a crash report.");
        (
            "Minecraft Crashed!".to_owned(),
            format!("{desc} (Exit code: {})", exit_code.unwrap_or(-1)),
        )
    } else if saw_mixin_error {
        (
            "Mod Mixin Conflict / Crash!".to_owned(),
            "A Fabric mod failed to apply a bytecode Mixin into Minecraft.".to_owned(),
        )
    } else if is_horizon {
        (
            "Horizon Exited Unexpectedly".to_owned(),
            format!(
                "Horizon exited with code {}. Check suspect mods or log details below.",
                exit_code.unwrap_or(-1)
            ),
        )
    } else {
        (
            "Game Crashed!".to_owned(),
            format!(
                "{} exited with code {}.",
                engine_label,
                exit_code.unwrap_or(-1)
            ),
        )
    };

    let offending = if is_horizon {
        match_mentioned_horizon_mods(home, &combined_for_scan)
    } else {
        match_mentioned_java_mods(&mods_dir, &BTreeSet::new(), &combined_for_scan)
    };

    let mut solutions = Vec::new();
    if !offending.is_empty() {
        solutions.push(
            "One or more installed mods appeared in the crash output — try clicking 'Disable' on a suspect mod below and relaunching."
                .into(),
        );
    }
    if saw_mixin_error {
        solutions.push(
            "Update your mods so their Minecraft target version matches this instance exactly."
                .into(),
        );
    }
    solutions.push(
        "Use 'Copy Crash Report' below if you want to share the stack trace with the mod author."
            .into(),
    );

    Some(CrashDiagnosis {
        instance_id: instance_id.to_owned(),
        instance_name: instance_name.to_owned(),
        engine_label: engine_label.to_owned(),
        is_horizon,
        kind: CrashKind::GameCrash,
        headline,
        summary,
        solutions,
        offending_mods: offending,
        raw_details: if !crash_report_text.is_empty() {
            crash_report_text
        } else {
            tail_text(log_lines, 150)
        },
        crash_file: recent_crash_report,
        mods_dir,
    })
}

fn parse_fabric_formatted_exception(
    lines: &[String],
) -> (String, Vec<String>, BTreeSet<String>) {
    let mut solutions = Vec::new();
    let mut mentioned_ids = BTreeSet::new();
    let mut in_solutions = false;
    let mut in_unmet = false;
    let mut summary =
        "Fabric Loader stopped startup because installed mods have unmet dependencies or conflicts."
            .to_owned();

    for raw in lines {
        let stripped = strip_log_prefix(raw).trim();
        if stripped.contains("FormattedException:") {
            if let Some((_, after)) = stripped.split_once("FormattedException:") {
                let msg = after.trim();
                if !msg.is_empty() && msg != "Incompatible mod set!" {
                    summary = msg.to_owned();
                }
            }
        }
        if stripped.starts_with("A potential solution has been determined")
            || stripped.starts_with("Potential solution")
        {
            in_solutions = true;
            in_unmet = false;
            continue;
        }
        if stripped.starts_with("Unmet dependency listing:")
            || stripped.starts_with("Inactive mods:")
            || stripped.starts_with("\tat ")
            || stripped.starts_with("at net.fabricmc.")
        {
            in_solutions = false;
            in_unmet = stripped.starts_with("Unmet dependency listing:");
            if stripped.starts_with("Unmet dependency listing:") {
                continue;
            }
        }
        if in_solutions {
            let bullet = stripped.trim_start_matches(['-', '*', '•', '\t', ' ']).trim();
            if !bullet.is_empty() {
                solutions.push(bullet.to_owned());
                extract_single_quoted_tokens(bullet, &mut mentioned_ids);
                extract_paren_tokens(bullet, &mut mentioned_ids);
            }
        } else if in_unmet {
            let bullet = stripped.trim_start_matches(['-', '*', '•', '\t', ' ']).trim();
            if !bullet.is_empty() {
                extract_single_quoted_tokens(bullet, &mut mentioned_ids);
                extract_paren_tokens(bullet, &mut mentioned_ids);
            }
        }
    }
    (summary, solutions, mentioned_ids)
}

fn strip_log_prefix(line: &str) -> &str {
    // Strip `[12:34:56] [main/ERROR]: ` if present so bullet detection works cleanly.
    if let Some((_, rhs)) = line.split_once("]: ") {
        rhs
    } else {
        line
    }
}

fn extract_single_quoted_tokens(s: &str, out: &mut BTreeSet<String>) {
    let mut rest = s;
    while let Some(a) = rest.find('\'') {
        let after = &rest[a + 1..];
        let Some(b) = after.find('\'') else {
            break;
        };
        let tok = after[..b].trim();
        if !tok.is_empty() {
            out.insert(tok.to_ascii_lowercase());
        }
        rest = &after[b + 1..];
    }
}

fn extract_paren_tokens(s: &str, out: &mut BTreeSet<String>) {
    let mut rest = s;
    while let Some(a) = rest.find('(') {
        let after = &rest[a + 1..];
        let Some(b) = after.find(')') else {
            break;
        };
        let tok = after[..b].trim();
        if !tok.is_empty() && !tok.contains(' ') {
            out.insert(tok.to_ascii_lowercase());
        }
        rest = &after[b + 1..];
    }
}

fn match_mentioned_java_mods(
    mods_dir: &Path,
    mentioned: &BTreeSet<String>,
    full_text: &str,
) -> Vec<SuspectMod> {
    let Ok(rd) = fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    let lower_text = full_text.to_ascii_lowercase();
    let mut out = Vec::new();

    for entry in rd.flatten() {
        let p = entry.path();
        let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !p.is_file() || !fname.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        let Some(scanned) = inspect_java_jar(&p, fname) else {
            continue;
        };
        if matches!(
            scanned.id.as_str(),
            "fabric-api" | "fabric" | "fabricloader"
        ) {
            continue;
        }
        let id_low = scanned.id.to_ascii_lowercase();
        let name_low = scanned.name.to_ascii_lowercase();
        let file_low = fname.to_ascii_lowercase();

        let hit = mentioned.contains(&id_low)
            || mentioned.contains(&name_low)
            || lower_text.contains(&format!("mod '{id_low}'"))
            || lower_text.contains(&format!("({id_low})"))
            || lower_text.contains(&format!("from mod {id_low}"))
            || lower_text.contains(&format!("mod:{id_low}"))
            || lower_text.contains(&file_low);

        if hit {
            out.push(SuspectMod {
                mod_id: scanned.id,
                display_name: format!("{} ({})", scanned.name, fname),
                reason: "Referenced in crash / dependency output.".into(),
                path: Some(p),
                is_horizon: false,
                disabled: false,
            });
        }
    }
    out
}

fn match_mentioned_horizon_mods(home: &Path, full_text: &str) -> Vec<SuspectMod> {
    let lower_text = full_text.to_ascii_lowercase();
    let state = hmods::read_mods_state(home);
    let mut out = Vec::new();
    for (sub, is_native) in [("horizon-mods", false), ("horizon-native-mods", true)] {
        let dir = home.join(sub);
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if let Ok(info) = hmods::inspect_any_mod_path(&p, is_native) {
                if !state.get(&info.id).copied().unwrap_or(true) {
                    continue;
                }
                if lower_text.contains(&info.id.to_ascii_lowercase())
                    || lower_text.contains(&info.name.to_ascii_lowercase())
                {
                    out.push(SuspectMod {
                        mod_id: info.id,
                        display_name: format!("{} ({})", info.name, info.runtime.badge()),
                        reason: "Referenced in Horizon output.".into(),
                        path: Some(p),
                        is_horizon: true,
                        disabled: false,
                    });
                }
            }
        }
    }
    out
}

fn newest_crash_report_since(crash_dir: &Path, since: SystemTime) -> Option<PathBuf> {
    let rd = fs::read_dir(crash_dir).ok()?;
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in rd.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(mtime) = meta.modified() else {
            continue;
        };
        if mtime >= since {
            if best.as_ref().is_none_or(|(prev, _)| mtime > *prev) {
                best = Some((mtime, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn tail_text(lines: &[String], max_lines: usize) -> String {
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn detects_forge_jar_and_missing_fabric_api_with_nested_jar_awareness() {
        let tmp = std::env::temp_dir().join(format!("fnh-crash-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let mods = tmp.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let opts = zip::write::SimpleFileOptions::default();

        // 1. Forge-only `.jar` in a Fabric instance -> `CrashKind::UnsupportedMod`
        let forge_jar = mods.join("jei-forge.jar");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&forge_jar).unwrap());
            z.start_file("META-INF/mods.toml", opts).unwrap();
            write!(z, "modLoader=\"javafml\"\n").unwrap();
            z.finish().unwrap();
        }
        let diag = check_prelaunch_compatibility("i1", "Fabric 1.21", &tmp, false, "1.21.4", true)
            .expect("should flag Forge jar");
        assert_eq!(diag.kind, CrashKind::UnsupportedMod);
        assert_eq!(diag.offending_mods.len(), 1);

        // Disable offending Forge jar in 1 click!
        disable_suspect_mod(&tmp, &diag.offending_mods[0]).unwrap();
        assert!(mods.join("jei-forge.jar.disabled").is_file());

        // 2. Fabric mod requiring `fabric-api` when `fabric-api` is missing -> `IncompatibleModSet`
        let zoom_jar = mods.join("zoomify.jar");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&zoom_jar).unwrap());
            z.start_file("fabric.mod.json", opts).unwrap();
            write!(
                z,
                r#"{{"schemaVersion":1,"id":"zoomify","name":"Zoomify","version":"2.0.0","depends":{{"fabric-api":">=0.100.0"}}}}"#
            )
            .unwrap();
            z.finish().unwrap();
        }
        let diag2 =
            check_prelaunch_compatibility("i1", "Fabric 1.21", &tmp, false, "1.21.4", true)
                .expect("should catch missing fabric-api");
        assert_eq!(diag2.kind, CrashKind::IncompatibleModSet);
        assert_eq!(diag2.headline, "Incompatible mod set!");
        assert_eq!(diag2.offending_mods[0].mod_id, "zoomify");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parses_fabric_formatted_exception_solutions() {
        let lines = vec![
            "[14:22:01] [main/ERROR]: Incompatible mod set!".to_owned(),
            "net.fabricmc.loader.impl.FormattedException: Mod resolution encountered an incompatible mod set!".to_owned(),
            "A potential solution has been determined, this may resolve your problem:".to_owned(),
            "\t - Install 'fabric-api', version 0.114.0 or later.".to_owned(),
            "Unmet dependency listing:".to_owned(),
            "\t - Mod 'Sodium Extra' (sodium-extra) 0.6.0 requires any version of fabric-api, which is missing!".to_owned(),
        ];
        let (_, solutions, ids) = parse_fabric_formatted_exception(&lines);
        assert_eq!(solutions.len(), 1);
        assert!(solutions[0].contains("fabric-api"));
        assert!(ids.contains("sodium-extra"));
    }
}
