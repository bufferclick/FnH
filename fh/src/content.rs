//! Per-instance content management: Mods (Fabric `.jar` + Horizon `.hmod`, `.hnmod`, folder mods,
//! and ported Java `.jar` mods), Modrinth & Horizon Registry search/install, Resource Packs,
//! Shader Packs, Instance Notes, Worlds & Datapacks, Servers (`servers.dat`), Screenshots,
//! Other Logs (`logs/` + `crash-reports/`), and Windows `.lnk` Desktop/Start-Menu shortcuts.

use horizon_launcher::browser::{
    self,
    details::{Filters, Sort},
    horizon as hreg,
    installed::Installed,
    java as hjava, java_instance,
    modrinth::Modrinth,
    Kind as BrowseKind, Project, Query, Source, SourceId,
};
use horizon_launcher::instances::{self as inst, Instance, Kind as InstKind};
use horizon_port_pack::{
    check::{self, Verdict},
    jar,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DISABLED_SUFFIX: &str = ".disabled";
const HORIZON_NATIVE_DIR: &str = "horizon-native-mods";
const NOTES_FILE: &str = "fnh-notes.txt";

/// Returns only real user-created/imported FNH instances (never the synthetic `Kind::Default`
/// slot), so a fresh install or factory reset starts empty just like JLac and every Horizon
/// instance owns its own strictly isolated `<FnH>/game/instances/<id>` directory.
pub fn user_instances() -> Vec<Instance> {
    inst::list()
        .into_iter()
        .filter(|i| i.kind != InstKind::Default)
        .collect()
}

/// Creates a strictly isolated Horizon instance, allowing even the name `"Horizon"` (which
/// `instances::create` normally reserves for the hidden `Kind::Default` slot).
pub fn create_horizon_instance(name: &str) -> Result<Instance, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("An instance needs a name.".into());
    }
    if user_instances()
        .iter()
        .any(|i| i.name.eq_ignore_ascii_case(trimmed))
    {
        return Err(format!(
            "There is already an instance called {trimmed}. Choose another name."
        ));
    }
    if trimmed.eq_ignore_ascii_case("horizon") {
        let created = inst::create("Horizon Instance", None)?;
        let _ = fs::create_dir_all(created.home.join(hjava::MODS_DIR));
        return inst::find(&created.id);
    }
    let created = inst::create(trimmed, None)?;
    let _ = fs::create_dir_all(created.home.join(hjava::MODS_DIR));
    Ok(created)
}

/// Creates a strictly isolated Java/Fabric instance, allowing even the name `"Horizon"`.
pub fn create_java_instance(name: &str, mc_version: &str) -> Result<Instance, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("An instance needs a name.".into());
    }
    if user_instances()
        .iter()
        .any(|i| i.name.eq_ignore_ascii_case(trimmed))
    {
        return Err(format!(
            "There is already an instance called {trimmed}. Choose another name."
        ));
    }
    let safe_name = if trimmed.eq_ignore_ascii_case("horizon") {
        format!("Horizon {mc_version}")
    } else {
        trimmed.to_owned()
    };
    let created = inst::create_java(&safe_name, mc_version)?;
    let _ = fs::create_dir_all(created.home.join("mods"));
    let _ = fs::create_dir_all(created.home.join("resourcepacks"));
    let _ = fs::create_dir_all(created.home.join("shaderpacks"));
    Ok(created)
}

// ================================================================================================
// Mods (Fabric / Java AND Horizon)
// ================================================================================================

/// What format an installed mod uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModFormat {
    /// Fabric/Quilt/NeoForge/Forge `.jar` in `<home>/mods/`.
    JavaJar,
    /// Horizon `.hmod` WebAssembly/content bundle in `<home>/horizon-mods/`.
    HorizonMod,
    /// Horizon folder mod (or ported Java `.jar`) in `<home>/horizon-mods/<id>/`.
    HorizonFolder,
    /// Horizon native mixin `.hnmod` in `<home>/horizon-native-mods/`.
    HorizonNative,
}

impl ModFormat {
    pub fn badge(self) -> &'static str {
        match self {
            ModFormat::JavaJar => "JAR",
            ModFormat::HorizonMod => "HMOD",
            ModFormat::HorizonFolder => "Ported",
            ModFormat::HorizonNative => "Native",
        }
    }
}

/// One row in `InstanceWindow -> Mods`.
#[derive(Clone, Debug)]
pub struct ModItem {
    /// Mod identifier (jar file name for Java, mod id for Horizon).
    pub id: String,
    pub name: String,
    pub version: String,
    pub file_label: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub format: ModFormat,
    pub source: String,
    pub browser_slug: Option<String>,
}

/// Primary mods directory opened by `View Folder` (created if absent so pasting always works).
pub fn mods_folder(instance: &Instance) -> PathBuf {
    let dir = if instance.java.is_some() {
        instance.home.join("mods")
    } else {
        instance.home.join(hjava::MODS_DIR)
    };
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Lists every mod in `instance`, covering both browser-installed and manually pasted/dropped mods.
pub fn list_mods(instance: &Instance) -> Vec<ModItem> {
    list_mods_in(&instance.home, instance.java.is_some())
}

/// Lists all mods inside `home` (`is_java = true` for Fabric/Java, `false` for Horizon).
pub fn list_mods_in(home: &Path, is_java: bool) -> Vec<ModItem> {
    if !is_java {
        // If a user manually pasted `.hmod` or `.hnmod` files into `<home>/mods/` (out of habit
        // from Minecraft/Fabric), automatically adopt them into `horizon-mods/` or
        // `horizon-native-mods/` and enable them so they show up and load immediately.
        adopt_misplaced_horizon_mods(home);
    }

    let installed = Installed::load(home);
    let mut rows = Vec::new();

    if is_java {
        if let Ok(entries) = fs::read_dir(home.join("mods")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let raw_name = entry.file_name().to_string_lossy().into_owned();
                let (enabled, clean_jar) = if let Some(base) = raw_name.strip_suffix(DISABLED_SUFFIX)
                {
                    if !base.to_ascii_lowercase().ends_with(".jar") {
                        continue;
                    }
                    (false, base.to_owned())
                } else if raw_name.to_ascii_lowercase().ends_with(".jar") {
                    (true, raw_name.clone())
                } else {
                    continue;
                };

                let rel = format!("mods/{clean_jar}");
                let tracked = installed.entries.iter().find(|e| e.file == rel);
                let (name, version, source, slug) = if let Some(e) = tracked {
                    (
                        e.title.clone(),
                        e.number.clone(),
                        e.source.label().to_owned(),
                        Some(e.slug.clone()),
                    )
                } else {
                    match jar::read(&path) {
                        Ok(info) => {
                            let title = if info.name.trim().is_empty() {
                                clean_jar.trim_end_matches(".jar").to_owned()
                            } else {
                                info.name
                            };
                            (title, info.version, "Local file".to_owned(), None)
                        }
                        Err(_) => (
                            clean_jar.trim_end_matches(".jar").to_owned(),
                            String::new(),
                            "Local file".to_owned(),
                            None,
                        ),
                    }
                };

                rows.push(ModItem {
                    id: clean_jar.clone(),
                    name,
                    version,
                    file_label: clean_jar,
                    path,
                    enabled,
                    format: ModFormat::JavaJar,
                    source,
                    browser_slug: slug,
                });
            }
        }
    } else {
        // 1. Scan `<home>/horizon-mods/` for `.hmod` files AND folder mods (including ported jars).
        let hmods_dir = home.join(hjava::MODS_DIR);
        if let Ok(entries) = fs::read_dir(&hmods_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let raw = entry.file_name().to_string_lossy().into_owned();
                if raw.starts_with('.') {
                    continue;
                }
                if path.is_file() && raw.to_ascii_lowercase().ends_with(".hmod") {
                    let id = raw[..raw.len() - 5].to_owned();
                    let rel = format!("{}/{raw}", hjava::MODS_DIR);
                    let tracked = installed.entries.iter().find(|e| e.file == rel);
                    let (name, version, source, slug) = match tracked {
                        Some(e) => (
                            e.title.clone(),
                            e.number.clone(),
                            e.source.label().to_owned(),
                            Some(e.slug.clone()),
                        ),
                        None => (id.clone(), String::new(), "Local .hmod".into(), None),
                    };
                    rows.push(ModItem {
                        enabled: hjava::is_enabled(home, &id),
                        id,
                        name,
                        version,
                        file_label: raw,
                        path,
                        format: ModFormat::HorizonMod,
                        source,
                        browser_slug: slug,
                    });
                } else if path.is_dir() {
                    let id = raw.clone();
                    let rel = format!("{}/{id}", hjava::MODS_DIR);
                    let tracked = installed.entries.iter().find(|e| e.file == rel);
                    let (name, version, source, slug) = match tracked {
                        Some(e) => {
                            let tag = match &e.verdict {
                                Some(v) => format!("{} ({v})", e.source.label()),
                                None => e.source.label().to_owned(),
                            };
                            (e.title.clone(), e.number.clone(), tag, Some(e.slug.clone()))
                        }
                        None => read_local_folder_mod_meta(&path, &id),
                    };
                    rows.push(ModItem {
                        enabled: hjava::is_enabled(home, &id),
                        id,
                        name,
                        version,
                        file_label: format!("{raw}/"),
                        path,
                        format: ModFormat::HorizonFolder,
                        source,
                        browser_slug: slug,
                    });
                }
            }
        }

        // 2. Scan `<home>/horizon-native-mods/` for `.hnmod` / `.hmod` native mixin bundles.
        let native_dir = home.join(HORIZON_NATIVE_DIR);
        if let Ok(entries) = fs::read_dir(&native_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let raw = entry.file_name().to_string_lossy().into_owned();
                let lower = raw.to_ascii_lowercase();
                let id = if lower.ends_with(".hnmod") {
                    raw[..raw.len() - 6].to_owned()
                } else if lower.ends_with(".hmod") {
                    raw[..raw.len() - 5].to_owned()
                } else {
                    continue;
                };
                let rel = format!("{HORIZON_NATIVE_DIR}/{raw}");
                let tracked = installed.entries.iter().find(|e| e.file == rel);
                let (name, version, source, slug) = match tracked {
                    Some(e) => (
                        e.title.clone(),
                        e.number.clone(),
                        "Horizon Native".into(),
                        Some(e.slug.clone()),
                    ),
                    None => (id.clone(), String::new(), "Local .hnmod".into(), None),
                };
                rows.push(ModItem {
                    enabled: hjava::is_enabled(home, &id),
                    id,
                    name,
                    version,
                    file_label: raw,
                    path,
                    format: ModFormat::HorizonNative,
                    source,
                    browser_slug: slug,
                });
            }
        }
    }

    rows.sort_by_key(|m| m.name.to_ascii_lowercase());
    rows
}

fn read_local_folder_mod_meta(dir: &Path, fallback_id: &str) -> (String, String, String, Option<String>) {
    if let Ok(text) = fs::read_to_string(dir.join("fnh-mod.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            let name = val["name"].as_str().unwrap_or(fallback_id).to_owned();
            let ver = val["version"].as_str().unwrap_or("").to_owned();
            let src = val["source"].as_str().unwrap_or("Ported JAR").to_owned();
            return (name, ver, src, None);
        }
    }
    (fallback_id.to_owned(), String::new(), "Folder mod".into(), None)
}

/// Automatically adopts `.hmod`, `.hnmod` or portable `.jar` files placed inside `<home>/mods/` on
/// a Horizon instance so pasting into `mods/` works seamlessly without confusion.
fn adopt_misplaced_horizon_mods(home: &Path) {
    let legacy = home.join("mods");
    let Ok(entries) = fs::read_dir(&legacy) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".hmod") {
            let dst_dir = home.join(hjava::MODS_DIR);
            let _ = fs::create_dir_all(&dst_dir);
            let dst = dst_dir.join(&name);
            if !dst.exists() && fs::rename(&path, &dst).is_ok() {
                let id = &name[..name.len() - 5];
                let _ = hjava::set_enabled(home, id, true);
            }
        } else if lower.ends_with(".hnmod") {
            let dst_dir = home.join(HORIZON_NATIVE_DIR);
            let _ = fs::create_dir_all(&dst_dir);
            let dst = dst_dir.join(&name);
            if !dst.exists() && fs::rename(&path, &dst).is_ok() {
                let id = &name[..name.len() - 6];
                let _ = hjava::set_enabled(home, id, true);
            }
        } else if lower.ends_with(".jar") {
            if add_horizon_mod_file(home, &path).is_ok() {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// Installs one or more dropped or selected mod files into `instance`, returning a human summary.
pub fn add_mod_files(instance: &Instance, files: &[PathBuf]) -> Result<String, String> {
    add_mod_files_to(&instance.home, instance.java.is_some(), files)
}

/// Installs mod files into `home` (`is_java = true` for Fabric/Java, `false` for Horizon).
pub fn add_mod_files_to(home: &Path, is_java: bool, files: &[PathBuf]) -> Result<String, String> {
    if files.is_empty() {
        return Err("No mod files selected.".into());
    }
    let mut added = Vec::new();
    let mut errors = Vec::new();

    for src in files {
        let res = if is_java {
            add_java_mod_file(home, src)
        } else {
            add_horizon_mod_file(home, src)
        };
        match res {
            Ok(label) => added.push(label),
            Err(err) => errors.push(err),
        }
    }

    match (added.is_empty(), errors.is_empty()) {
        (true, _) => Err(errors.join(" | ")),
        (false, true) if added.len() == 1 => Ok(format!("Added mod: {}.", added[0])),
        (false, true) => Ok(format!("Added {} mods: {}.", added.len(), added.join(", "))),
        (false, false) => Ok(format!(
            "Added {} ({}); skipped: {}",
            added.len(),
            added.join(", "),
            errors.join(" | ")
        )),
    }
}

fn add_java_mod_file(home: &Path, src: &Path) -> Result<String, String> {
    if !src.is_file() {
        return Err(format!("{} is not a .jar file", src.display()));
    }
    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name.".to_owned())?;
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".hmod") || lower.ends_with(".hnmod") {
        return Err(format!(
            "{file_name} is a Horizon mod — add it to a Horizon instance instead"
        ));
    }
    if !lower.ends_with(".jar") && !lower.ends_with(".jar.disabled") {
        return Err(format!(
            "{file_name} is not a .jar mod (Fabric loads .jar files)"
        ));
    }
    let mods = home.join("mods");
    fs::create_dir_all(&mods).map_err(|e| format!("{}: {e}", mods.display()))?;
    let dest = mods.join(file_name);
    if src != dest {
        fs::copy(src, &dest).map_err(|e| format!("Could not copy {file_name}: {e}"))?;
    }
    let display = jar::read(&dest)
        .ok()
        .map(|j| j.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| file_name.trim_end_matches(".jar").to_owned());
    Ok(display)
}

fn add_horizon_mod_file(home: &Path, src: &Path) -> Result<String, String> {
    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid mod path.".to_owned())?;
    let lower = file_name.to_ascii_lowercase();

    // 1. Folder mod (e.g. contains pack.mcmeta or data/ or assets/).
    if src.is_dir() {
        let id = sanitize_mod_id(file_name);
        let dst = home.join(hjava::MODS_DIR).join(&id);
        copy_dir_recursive(src, &dst)?;
        hjava::set_enabled(home, &id, true)?;
        return Ok(id);
    }

    // 2. Native Horizon `.hmod` WebAssembly / content mod.
    if lower.ends_with(".hmod") {
        let id = file_name[..file_name.len() - 5].to_owned();
        let dir = home.join(hjava::MODS_DIR);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let dst = dir.join(file_name);
        if src != dst {
            fs::copy(src, &dst).map_err(|e| format!("Could not copy {file_name}: {e}"))?;
        }
        hjava::set_enabled(home, &id, true)?;
        return Ok(id);
    }

    // 3. Horizon `.hnmod` native mixin mod.
    if lower.ends_with(".hnmod") {
        let id = file_name[..file_name.len() - 6].to_owned();
        let dir = home.join(HORIZON_NATIVE_DIR);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let dst = dir.join(file_name);
        if src != dst {
            fs::copy(src, &dst).map_err(|e| format!("Could not copy {file_name}: {e}"))?;
        }
        hjava::set_enabled(home, &id, true)?;
        return Ok(format!("{id} (native mod)"));
    }

    // 4. Java `.jar` mod: run Horizon's own `horizon-port-pack` converter to carry blocks, items,
    //    recipes, loot, tags & worldgen into `horizon-mods/<id>/` and turn it on!
    if lower.ends_with(".jar") {
        let stage = std::env::temp_dir().join(format!(
            "fnh-port-{}-{}",
            std::process::id(),
            now_secs()
        ));
        let _ = fs::remove_dir_all(&stage);
        let checked = check::check(src, &stage);
        let result = match checked {
            Ok(info) if matches!(info.verdict, Verdict::Works | Verdict::Partly) => {
                let carry = stage.join("carry");
                let id = if info.mod_id.trim().is_empty() {
                    sanitize_mod_id(file_name.trim_end_matches(".jar"))
                } else {
                    info.mod_id.clone()
                };
                let dst = home.join(hjava::MODS_DIR).join(&id);
                let _ = fs::remove_dir_all(&dst);
                copy_dir_recursive(&carry, &dst)?;
                let title = if info.name.trim().is_empty() {
                    id.clone()
                } else {
                    info.name.clone()
                };
                let meta = serde_json::json!({
                    "name": title,
                    "version": info.version,
                    "source": format!("Ported JAR ({})", info.verdict.label()),
                });
                let _ = fs::write(dst.join("fnh-mod.json"), meta.to_string());
                hjava::set_enabled(home, &id, true)?;
                Ok(format!("{title} ({})", info.verdict.label()))
            }
            Ok(info) => {
                let name = if info.name.trim().is_empty() {
                    file_name
                } else {
                    &info.name
                };
                Err(format!(
                    "{name} is a Java code mod that cannot run on Horizon ({}) — drop it into a Fabric instance to run it in full, or install a Horizon .hmod mod",
                    info.summary
                ))
            }
            Err(e) => Err(format!("Could not inspect {file_name}: {e}")),
        };
        let _ = fs::remove_dir_all(&stage);
        return result;
    }

    Err(format!(
        "{file_name}: Horizon accepts .hmod, .hnmod, or portable .jar mods"
    ))
}

/// Toggles a mod enabled/disabled.
pub fn set_mod_enabled(home: &Path, item: &ModItem, on: bool) -> Result<(), String> {
    match item.format {
        ModFormat::JavaJar => {
            let mods = home.join("mods");
            let active = mods.join(&item.file_label);
            let disabled = mods.join(format!("{}{DISABLED_SUFFIX}", item.file_label));
            let (from, to) = if on {
                (disabled, active)
            } else {
                (active, disabled)
            };
            if from.exists() {
                fs::rename(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
            }
            Ok(())
        }
        ModFormat::HorizonMod | ModFormat::HorizonFolder | ModFormat::HorizonNative => {
            hjava::set_enabled(home, &item.id, on)
        }
    }
}

/// Removes a mod from `instance`.
pub fn remove_mod(home: &Path, item: &ModItem) -> Result<(), String> {
    if let Some(slug) = &item.browser_slug {
        if item.format == ModFormat::JavaJar {
            if java_instance::remove(home, slug).is_ok() {
                return Ok(());
            }
        } else if browser::installed::remove(home, slug).is_ok() {
            let _ = hjava::set_enabled(home, &item.id, false);
            return Ok(());
        }
    }
    if item.path.is_dir() {
        fs::remove_dir_all(&item.path).map_err(|e| format!("{}: {e}", item.path.display()))?;
    } else if item.path.exists() {
        fs::remove_file(&item.path).map_err(|e| format!("{}: {e}", item.path.display()))?;
    }
    if item.format != ModFormat::JavaJar {
        let _ = hjava::set_enabled(home, &item.id, false);
    }
    Ok(())
}

// ================================================================================================
// Modrinth & Horizon Registry Browser
// ================================================================================================

/// Searches Modrinth or Horizon Registry for mods, resource packs, shader packs, or modpacks.
pub fn search_projects(
    source_id: SourceId,
    kind: BrowseKind,
    text: &str,
    sort: Sort,
    java_target: Option<(&str, Option<&str>)>,
) -> Result<Vec<Project>, String> {
    let source = browser::source(source_id)
        .ok_or_else(|| format!("{} is not available.", source_id.label()))?;
    let mut query = if let (SourceId::Modrinth, Some((mc, loader))) = (source_id, java_target) {
        let target = java_instance::Target {
            minecraft: mc.to_owned(),
            loader: loader.map(str::to_owned),
        };
        target.query(text, kind)
    } else {
        Query::new(text, kind)
    };
    query.limit = 30;
    query.filters = Filters {
        sort,
        ..query.filters
    };
    source.search(&query)
}

/// Installs a searched project (and required dependencies) into `instance`.
pub fn install_project(
    instance: &Instance,
    project: &Project,
    progress: &dyn Fn(&str, f32),
) -> Result<String, String> {
    if let Some(java) = &instance.java {
        if project.kind == BrowseKind::Shader {
            return install_modrinth_shader(&instance.home, &java.minecraft, project, progress);
        }
        let target = java_instance::Target {
            minecraft: java.minecraft.clone(),
            loader: java.loader.as_ref().map(|l| l.kind.id().to_owned()),
        };
        let done = java_instance::install(&Modrinth, &target, project, &instance.home, progress)?;
        let count = done.len();
        if count > 1 {
            Ok(format!(
                "Installed {} (+ {} required {}).",
                project.title,
                count - 1,
                if count == 2 {
                    "dependency"
                } else {
                    "dependencies"
                }
            ))
        } else {
            Ok(format!("Installed {}.", project.title))
        }
    } else {
        match project.source {
            SourceId::Horizon => {
                hreg::install(project, &instance.home, progress)?;
                Ok(format!("Installed {} from Horizon Registry.", project.title))
            }
            SourceId::Modrinth => {
                if project.kind == BrowseKind::Mod {
                    let done = hjava::install(&Modrinth, project, false, &instance.home, progress)?;
                    Ok(format!(
                        "Ported & installed {} ({} file{}).",
                        project.title,
                        done.len(),
                        if done.len() == 1 { "" } else { "s" }
                    ))
                } else {
                    browser::installed::install(&Modrinth, project, false, &instance.home, progress)?;
                    Ok(format!("Installed {}.", project.title))
                }
            }
            SourceId::CurseForge => Err(browser::CURSEFORGE_SOON.into()),
        }
    }
}

fn install_modrinth_shader(
    home: &Path,
    minecraft: &str,
    project: &Project,
    progress: &dyn Fn(&str, f32),
) -> Result<String, String> {
    let versions = Modrinth.versions(project)?;
    let chosen = versions
        .iter()
        .find(|v| v.blocked.is_none() && v.file().is_some() && v.game_versions.iter().any(|g| g == minecraft))
        .or_else(|| versions.iter().find(|v| v.blocked.is_none() && v.file().is_some()))
        .ok_or_else(|| format!("No downloadable shader pack found for {}.", project.title))?;
    let file = chosen.file().ok_or("Shader version has no file.")?;
    let dir = home.join("shaderpacks");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    progress(&format!("Downloading {}", project.title), 0.2);
    Modrinth.fetch(file, &dir, &|p| progress(&format!("Downloading {}", project.title), p))?;
    Ok(format!("Installed shader pack {}.", project.title))
}

// ================================================================================================
// Resource Packs & Shader Packs
// ================================================================================================

#[derive(Clone, Debug)]
pub struct PackItem {
    pub name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub size_bytes: u64,
    pub is_dir: bool,
}

pub fn list_packs(home: &Path, folder: &str) -> Vec<PackItem> {
    let dir = home.join(folder);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let raw = entry.file_name().to_string_lossy().into_owned();
        if raw.starts_with('.') {
            continue;
        }
        let is_dir = path.is_dir();
        let (enabled, clean) = if let Some(base) = raw.strip_suffix(DISABLED_SUFFIX) {
            (false, base.to_owned())
        } else {
            (true, raw)
        };
        let lower = clean.to_ascii_lowercase();
        if !is_dir && !lower.ends_with(".zip") && !lower.ends_with(".hshader") && !lower.ends_with(".jar") {
            continue;
        }
        let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        out.push(PackItem {
            name: clean,
            path,
            enabled,
            size_bytes,
            is_dir,
        });
    }
    out.sort_by_key(|p| p.name.to_ascii_lowercase());
    out
}

pub fn add_pack_files(home: &Path, folder: &str, files: &[PathBuf]) -> Result<String, String> {
    let dir = home.join(folder);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut count = 0usize;
    for src in files {
        let Some(name) = src.file_name() else {
            continue;
        };
        let dst = dir.join(name);
        if src.is_dir() {
            copy_dir_recursive(src, &dst)?;
            count += 1;
        } else if src.is_file() {
            fs::copy(src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
            count += 1;
        }
    }
    if count == 0 {
        Err("No valid pack files were selected.".into())
    } else {
        Ok(format!("Added {count} pack{}.", if count == 1 { "" } else { "s" }))
    }
}

pub fn set_pack_enabled(item: &PackItem, on: bool) -> Result<(), String> {
    let Some(parent) = item.path.parent() else {
        return Err("Invalid pack path.".into());
    };
    let active = parent.join(&item.name);
    let disabled = parent.join(format!("{}{DISABLED_SUFFIX}", item.name));
    let (from, to) = if on {
        (disabled, active)
    } else {
        (active, disabled)
    };
    if from.exists() {
        fs::rename(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    }
    Ok(())
}

pub fn remove_pack(item: &PackItem) -> Result<(), String> {
    if item.is_dir {
        fs::remove_dir_all(&item.path).map_err(|e| format!("{}: {e}", item.path.display()))
    } else {
        fs::remove_file(&item.path).map_err(|e| format!("{}: {e}", item.path.display()))
    }
}

// ================================================================================================
// Notes, Screenshots & Other Logs
// ================================================================================================

pub fn load_notes(home: &Path) -> String {
    fs::read_to_string(home.join(NOTES_FILE)).unwrap_or_default()
}

pub fn save_notes(home: &Path, text: &str) -> Result<(), String> {
    fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
    let path = home.join(NOTES_FILE);
    if text.trim().is_empty() {
        let _ = fs::remove_file(&path);
        Ok(())
    } else {
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[derive(Clone, Debug)]
pub struct ScreenshotItem {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: u64,
}

pub fn list_screenshots(home: &Path) -> Vec<ScreenshotItem> {
    let dir = home.join("screenshots");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !crate::inst_icons::is_image_file(&path) {
            continue;
        }
        let meta = fs::metadata(&path).ok();
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out.push(ScreenshotItem {
            name: entry.file_name().to_string_lossy().into_owned(),
            path,
            size_bytes: meta.map(|m| m.len()).unwrap_or(0),
            modified,
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

#[derive(Clone, Debug)]
pub struct LogFileItem {
    pub label: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: u64,
}

pub fn list_log_files(home: &Path) -> Vec<LogFileItem> {
    let mut out = Vec::new();
    for (sub, prefix) in [("logs", "logs"), ("crash-reports", "crash-reports")] {
        let Ok(entries) = fs::read_dir(home.join(sub)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".log") && !name.ends_with(".txt") {
                continue;
            }
            let meta = fs::metadata(&path).ok();
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(LogFileItem {
                label: format!("{prefix}/{name}"),
                path,
                size_bytes: meta.map(|m| m.len()).unwrap_or(0),
                modified,
            });
        }
    }
    out.sort_by(|a, b| {
        let a_latest = a.label == "logs/latest.log";
        let b_latest = b.label == "logs/latest.log";
        b_latest.cmp(&a_latest).then_with(|| b.modified.cmp(&a.modified))
    });
    out
}

pub fn read_log_tail(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let slice = if bytes.len() > 512 * 1024 {
        &bytes[bytes.len() - 512 * 1024..]
    } else {
        &bytes[..]
    };
    let text = String::from_utf8_lossy(slice);
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(max_lines.max(50));
    all[start..].iter().map(|s| (*s).to_owned()).collect()
}

// ================================================================================================
// Desktop / Start Menu Shortcut Creation (JLac `CreateShortcutDialog`)
// ================================================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutTarget {
    Desktop,
    StartMenu,
}

pub fn create_instance_shortcut(
    instance: &Instance,
    shortcut_name: &str,
    target: ShortcutTarget,
    offline: bool,
) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Could not find FH.exe: {e}"))?;
    let safe: String = shortcut_name
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let safe = if safe.trim().is_empty() {
        instance.name.clone()
    } else {
        safe
    };
    let flag = if offline { "--launch-offline" } else { "--launch" };

    #[cfg(windows)]
    {
        let dir = match target {
            ShortcutTarget::Desktop => dirs::desktop_dir()
                .ok_or_else(|| "Could not find Desktop folder.".to_owned())?,
            ShortcutTarget::StartMenu => dirs::data_dir()
                .map(|d| d.join("Microsoft\\Windows\\Start Menu\\Programs"))
                .ok_or_else(|| "Could not find Start Menu folder.".to_owned())?,
        };
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let lnk = dir.join(format!("{safe}.lnk"));
        let esc = |s: &str| s.replace('\'', "''");
        let work = exe
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .into_owned();
        let script = format!(
            "$ws = New-Object -ComObject WScript.Shell; \
             $s = $ws.CreateShortcut('{}'); \
             $s.TargetPath = '{}'; \
             $s.Arguments = '{} \"{}\"'; \
             $s.WorkingDirectory = '{}'; \
             $s.IconLocation = '{},0'; \
             $s.Save()",
            esc(&lnk.to_string_lossy()),
            esc(&exe.to_string_lossy()),
            flag,
            esc(&instance.id).replace('"', ""),
            esc(&work),
            esc(&exe.to_string_lossy()),
        );
        let status = horizon_launcher::process::command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .map_err(|e| format!("Could not create shortcut: {e}"))?;
        if status.success() && lnk.is_file() {
            Ok(lnk)
        } else {
            Err("Windows PowerShell declined to save the .lnk shortcut.".into())
        }
    }

    #[cfg(not(windows))]
    {
        let dir = match target {
            ShortcutTarget::Desktop => {
                dirs::desktop_dir().unwrap_or_else(|| crate::paths::root().join("shortcuts"))
            }
            ShortcutTarget::StartMenu => dirs::data_dir()
                .map(|d| d.join("applications"))
                .unwrap_or_else(|| crate::paths::root().join("shortcuts")),
        };
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join(format!("fnh-{safe}.desktop"));
        let body = format!(
            "[Desktop Entry]\nType=Application\nName={safe}\nExec=\"{}\" {flag} \"{}\"\nCategories=Game;\n",
            exe.display(),
            instance.id
        );
        fs::write(&file, body).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(file)
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    if bytes as f64 >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes as f64 >= KIB {
        format!("{:.0} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn sanitize_mod_id(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = s.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "mod".into()
    } else {
        trimmed
    }
}

pub fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
    }
    Ok(())
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_and_horizon_mods_install_toggle_and_remove() {
        let base = std::env::temp_dir().join(format!("fnh-mods-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let staging = base.join("incoming");
        fs::create_dir_all(&staging).unwrap();

        // 1. Fabric instance: add .jar, toggle off/on, remove.
        let fabric_home = base.join("fabric-inst");
        let jar_src = staging.join("example-mod-1.0.jar");
        fs::write(&jar_src, b"not-a-real-zip").unwrap();
        add_mod_files_to(&fabric_home, true, std::slice::from_ref(&jar_src)).unwrap();
        let mods = list_mods_in(&fabric_home, true);
        assert_eq!(mods.len(), 1);
        assert!(mods[0].enabled);
        set_mod_enabled(&fabric_home, &mods[0], false).unwrap();
        let mods = list_mods_in(&fabric_home, true);
        assert!(!mods[0].enabled);
        set_mod_enabled(&fabric_home, &mods[0], true).unwrap();
        let mods = list_mods_in(&fabric_home, true);
        assert!(mods[0].enabled);
        remove_mod(&fabric_home, &mods[0]).unwrap();
        assert!(list_mods_in(&fabric_home, true).is_empty());

        // 2. Horizon instance: add .hmod, add .hnmod, AND paste .hmod into legacy `mods/` folder.
        let hz_home = base.join("horizon-inst");
        let hmod_src = staging.join("cool_blocks.hmod");
        let hnmod_src = staging.join("fast_chunk.hnmod");
        fs::write(&hmod_src, b"hmod").unwrap();
        fs::write(&hnmod_src, b"hnmod").unwrap();
        add_mod_files_to(&hz_home, false, &[hmod_src, hnmod_src]).unwrap();

        // Also simulate user manually pasting a `.hmod` into `<instance>/mods/`.
        fs::create_dir_all(hz_home.join("mods")).unwrap();
        fs::write(hz_home.join("mods/pasted_mod.hmod"), b"hmod2").unwrap();

        let hz_mods = list_mods_in(&hz_home, false);
        assert_eq!(hz_mods.len(), 3);
        assert!(hz_mods.iter().all(|m| m.enabled));

        set_mod_enabled(&hz_home, &hz_mods[0], false).unwrap();
        let hz_mods = list_mods_in(&hz_home, false);
        assert!(!hz_mods[0].enabled);

        let _ = fs::remove_dir_all(&base);
    }
}
