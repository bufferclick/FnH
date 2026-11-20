//! Instance icon presets (the complete JLac icon library plus `default` FNH and `horizon`) and
//! custom user-imported image icons stored in `<FnH>/icons/`.
//!
//! Every preset is compiled directly into `FH.exe` as a crisp 128x128 RGBA PNG so zero external
//! files are required. Users can also import any `.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.bmp`
//! or `.ico` image (via `Add Icon`, drag-and-drop into the Icon Picker window, or dropping files
//! straight into `<FnH>/icons/`), select it for any Fabric or Horizon instance, or remove it.

use gpui::{div, img, prelude::*, px, Div, Image, ImageFormat, ObjectFit};
use horizon_launcher::instances::Instance;
use image::imageops::FilterType;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Filter categories in `Select Icon`, matching JLac's `IconPickerDialog`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconCategory {
    Any,
    Default,
    Legacy,
    Custom,
}

impl IconCategory {
    pub const ALL: [IconCategory; 4] = [
        IconCategory::Any,
        IconCategory::Default,
        IconCategory::Legacy,
        IconCategory::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IconCategory::Any => "Any",
            IconCategory::Default => "Default",
            IconCategory::Legacy => "Legacy",
            IconCategory::Custom => "Custom",
        }
    }
}

/// One built-in preset icon compiled into `FH.exe`.
pub struct BuiltinIcon {
    pub key: &'static str,
    pub label: &'static str,
    pub legacy: bool,
    pub png: &'static [u8],
}

/// One selectable icon entry (built-in preset or user custom icon in `<FnH>/icons/`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconEntry {
    /// Stored key (`"grass"`, `"fabricmc"`, `"custom:my_pack"`, ...).
    pub key: String,
    /// Human-readable label shown under the 48x48 preview.
    pub label: String,
    pub category: IconCategory,
}

macro_rules! preset {
    ($key:literal, $label:literal, $legacy:literal) => {
        BuiltinIcon {
            key: $key,
            label: $label,
            legacy: $legacy,
            png: include_bytes!(concat!("../icons/instances/", $key, ".png")),
        }
    };
}

/// All 61 built-in instance icon presets: FNH + Horizon + every single JLac default & legacy icon.
pub const BUILTINS: &[BuiltinIcon] = &[
    // --- Default (modern scalable JLac set + FNH + Horizon) ---
    preset!("default", "FnH", false),
    preset!("horizon", "Horizon", false),
    preset!("fabricmc", "Fabric", false),
    preset!("grass", "Grass", false),
    preset!("dirt", "Dirt", false),
    preset!("stone", "Stone", false),
    preset!("planks", "Planks", false),
    preset!("brick", "Brick", false),
    preset!("iron", "Iron", false),
    preset!("gold", "Gold", false),
    preset!("diamond", "Diamond", false),
    preset!("tnt", "TNT", false),
    preset!("creeper", "Creeper", false),
    preset!("squarecreeper", "Square Creeper", false),
    preset!("steve", "Steve", false),
    preset!("herobrine", "Herobrine", false),
    preset!("skeleton", "Skeleton", false),
    preset!("enderman", "Enderman", false),
    preset!("chicken", "Chicken", false),
    preset!("bee", "Bee", false),
    preset!("fox", "Fox", false),
    preset!("enderpearl", "Ender Pearl", false),
    preset!("netherstar", "Nether Star", false),
    preset!("flame", "Flame", false),
    preset!("meat", "Meat", false),
    preset!("gear", "Gear", false),
    preset!("magitech", "Magitech", false),
    preset!("modrinth", "Modrinth", false),
    preset!("jlac", "JLac", false),
    preset!("quiltmc", "Quilt", false),
    preset!("neoforged", "NeoForge", false),
    preset!("ftb_logo", "FTB", false),
    // --- Legacy (classic JLac / MultiMC set) ---
    preset!("grass_legacy", "Grass (Legacy)", true),
    preset!("dirt_legacy", "Dirt (Legacy)", true),
    preset!("stone_legacy", "Stone (Legacy)", true),
    preset!("planks_legacy", "Planks (Legacy)", true),
    preset!("brick_legacy", "Brick (Legacy)", true),
    preset!("iron_legacy", "Iron (Legacy)", true),
    preset!("gold_legacy", "Gold (Legacy)", true),
    preset!("diamond_legacy", "Diamond (Legacy)", true),
    preset!("tnt_legacy", "TNT (Legacy)", true),
    preset!("creeper_legacy", "Creeper (Legacy)", true),
    preset!("squarecreeper_legacy", "Square Creeper (Legacy)", true),
    preset!("steve_legacy", "Steve (Legacy)", true),
    preset!("herobrine_legacy", "Herobrine (Legacy)", true),
    preset!("skeleton_legacy", "Skeleton (Legacy)", true),
    preset!("enderman_legacy", "Enderman (Legacy)", true),
    preset!("chicken_legacy", "Chicken (Legacy)", true),
    preset!("bee_legacy", "Bee (Legacy)", true),
    preset!("fox_legacy", "Fox (Legacy)", true),
    preset!("enderpearl_legacy", "Ender Pearl (Legacy)", true),
    preset!("netherstar_legacy", "Nether Star (Legacy)", true),
    preset!("flame_legacy", "Flame (Legacy)", true),
    preset!("meat_legacy", "Meat (Legacy)", true),
    preset!("gear_legacy", "Gear (Legacy)", true),
    preset!("magitech_legacy", "Magitech (Legacy)", true),
    preset!("infinity_legacy", "Infinity (Legacy)", true),
    preset!("ftb_logo_legacy", "FTB (Legacy)", true),
    preset!("ftb_glow", "FTB Glow", true),
    preset!("forge", "Forge", true),
    preset!("liteloader", "LiteLoader", true),
];

/// Supported image file extensions for custom icons.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico"];

/// Prefix for custom icon keys saved in instance settings.
pub const CUSTOM_PREFIX: &str = "custom:";

static IMAGE_CACHE: OnceLock<Mutex<HashMap<String, Arc<Image>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Arc<Image>>> {
    IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Directory holding custom user icons (`<FnH>/icons`).
pub fn icons_dir() -> PathBuf {
    crate::paths::icons_dir()
}

/// Clears cached decoded icon images so newly dropped/edited files reload immediately.
pub fn clear_cache() {
    if let Ok(mut map) = cache().lock() {
        map.clear();
    }
}

/// Whether `path` has an image extension FNH can import as an instance icon.
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.as_str()))
}

/// Returns the default preset key for an instance when none was explicitly picked.
pub fn default_key_for(instance: &Instance) -> &'static str {
    match &instance.java {
        Some(java) => match java.loader.as_ref().map(|l| l.kind.id()) {
            Some("fabric") => "fabricmc",
            Some("quilt") => "quiltmc",
            Some("neoforge") => "neoforged",
            Some("forge") => "forge",
            _ => "grass",
        },
        None => "horizon",
    }
}

/// Lists all selectable icons (all user custom icons first when discovered, plus all 61 presets),
/// optionally filtered by `category` and `search`.
pub fn list_icons(category: IconCategory, search: &str) -> Vec<IconEntry> {
    let needle = search.trim().to_ascii_lowercase();
    let matches = |key: &str, label: &str| {
        needle.is_empty()
            || key.to_ascii_lowercase().contains(&needle)
            || label.to_ascii_lowercase().contains(&needle)
    };

    let mut out = Vec::new();

    if matches!(category, IconCategory::Any | IconCategory::Custom) {
        for (stem, _) in scan_custom_files() {
            let key = format!("{CUSTOM_PREFIX}{stem}");
            if matches(&stem, &stem) {
                out.push(IconEntry {
                    key,
                    label: stem,
                    category: IconCategory::Custom,
                });
            }
        }
    }

    for b in BUILTINS {
        let cat = if b.legacy {
            IconCategory::Legacy
        } else {
            IconCategory::Default
        };
        if category != IconCategory::Any && category != cat {
            continue;
        }
        if matches(b.key, b.label) {
            out.push(IconEntry {
                key: b.key.to_owned(),
                label: b.label.to_owned(),
                category: cat,
            });
        }
    }

    out
}

/// Scans `<FnH>/icons/` for user image files sorted alphabetically by stem.
fn scan_custom_files() -> Vec<(String, PathBuf)> {
    let dir = icons_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_image_file(&path) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let stem = stem.trim().to_owned();
        if stem.is_empty() {
            continue;
        }
        // Prefer `.png` if duplicate stems exist.
        if let Some((_, existing)) = found.iter_mut().find(|(s, _)| s.eq_ignore_ascii_case(&stem)) {
            if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                *existing = path;
            }
        } else {
            found.push((stem, path));
        }
    }
    found.sort_by_key(|(s, _)| s.to_ascii_lowercase());
    found
}

/// Sanitizes an arbitrary filename stem into a safe icon name.
fn sanitize_stem(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_owned();
    if trimmed.is_empty() {
        "icon".to_owned()
    } else {
        trimmed
    }
}

/// Normalizes any loaded `DynamicImage` into a crisp 128..=256px RGBA PNG buffer.
/// Tiny pixel-art icons (<= 64px) scale up with `Nearest` so Minecraft block pixels stay sharp.
fn encode_normalized_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let decoded = image::load_from_memory(bytes).map_err(|e| format!("Invalid image file: {e}"))?;
    let (w, h) = (decoded.width(), decoded.height());
    if w == 0 || h == 0 {
        return Err("Image has zero dimensions.".into());
    }
    let resized = if w.max(h) <= 64 {
        let scale = (128 / w.max(h)).max(1);
        decoded.resize_exact(w * scale, h * scale, FilterType::Nearest)
    } else if w.max(h) > 256 {
        decoded.resize(256, 256, FilterType::Lanczos3)
    } else {
        decoded
    };
    let mut out = Vec::new();
    resized
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("Could not encode PNG icon: {e}"))?;
    Ok(out)
}

/// Imports a user image file into `<FnH>/icons/<name>.png` and returns its icon key (`custom:...`).
pub fn import_custom_icon(source: &Path) -> Result<String, String> {
    import_custom_icon_into(source, &icons_dir())
}

/// Imports `source` into `dir` (testable without touching global paths).
pub fn import_custom_icon_into(source: &Path, dir: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(source).map_err(|e| format!("Could not read {}: {e}", source.display()))?;
    let png = encode_normalized_png(&bytes)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    let base = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_stem)
        .unwrap_or_else(|| "icon".into());
    let stem = (1usize..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}_{n}")
            }
        })
        .find(|candidate| !dir.join(format!("{candidate}.png")).exists())
        .unwrap_or(base);

    let dest = dir.join(format!("{stem}.png"));
    std::fs::write(&dest, png).map_err(|e| format!("Could not save {}: {e}", dest.display()))?;

    let key = format!("{CUSTOM_PREFIX}{stem}");
    if let Ok(mut map) = cache().lock() {
        map.remove(&key);
    }
    Ok(key)
}

/// Deletes a custom icon (`custom:<name>`) from `<FnH>/icons/`.
pub fn remove_custom_icon(key: &str) -> Result<(), String> {
    let Some(stem) = key.strip_prefix(CUSTOM_PREFIX) else {
        return Err("Built-in preset icons cannot be removed.".into());
    };
    let mut removed = false;
    for (found_stem, path) in scan_custom_files() {
        if found_stem.eq_ignore_ascii_case(stem) {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
            removed = true;
        }
    }
    if let Ok(mut map) = cache().lock() {
        map.remove(key);
    }
    if removed {
        Ok(())
    } else {
        Err(format!("Custom icon '{stem}' was not found."))
    }
}

/// Whether `key` refers to a removable user-imported custom icon.
pub fn is_custom_key(key: &str) -> bool {
    key.starts_with(CUSTOM_PREFIX)
}

/// Resolves `key` to a cached `Arc<gpui::Image>`, falling back cleanly to `"default"` if missing.
pub fn resolve_image(key: &str) -> Arc<Image> {
    let normalized = if key.trim().is_empty() {
        "default"
    } else {
        key.trim()
    };
    if let Ok(map) = cache().lock() {
        if let Some(found) = map.get(normalized) {
            return found.clone();
        }
    }

    let png_bytes: Vec<u8> = if let Some(stem) = normalized.strip_prefix(CUSTOM_PREFIX) {
        load_custom_png(stem).unwrap_or_else(|| builtin_png("default").to_vec())
    } else if let Some(b) = BUILTINS.iter().find(|b| b.key == normalized) {
        b.png.to_vec()
    } else {
        // Also allow bare stem matching a custom file for JLac-style convenience.
        load_custom_png(normalized).unwrap_or_else(|| builtin_png("default").to_vec())
    };

    let image = Arc::new(Image::from_bytes(ImageFormat::Png, png_bytes));
    if let Ok(mut map) = cache().lock() {
        map.insert(normalized.to_owned(), image.clone());
    }
    image
}

fn builtin_png(key: &str) -> &'static [u8] {
    BUILTINS
        .iter()
        .find(|b| b.key == key)
        .map(|b| b.png)
        .unwrap_or(BUILTINS[0].png)
}

fn load_custom_png(stem: &str) -> Option<Vec<u8>> {
    let (_, path) = scan_custom_files()
        .into_iter()
        .find(|(s, _)| s.eq_ignore_ascii_case(stem))?;
    let bytes = std::fs::read(&path).ok()?;
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        // Fast path: already PNG on disk.
        Some(bytes)
    } else {
        encode_normalized_png(&bytes).ok()
    }
}

/// Renders the instance icon for `key` at `size` logical pixels (preserving aspect ratio & alpha).
pub fn render_icon(key: &str, size: f32) -> Div {
    let image = resolve_image(key);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .child(img(image).size(px(size)).object_fit(ObjectFit::Contain))
}

/// Loads a screenshot thumbnail from disk normalized to a small cached PNG so gallery scrolling
/// stays butter-smooth.
pub fn load_thumbnail(path: &Path, max_edge: u32) -> Option<Arc<Image>> {
    let cache_key = format!("thumb:{max_edge}:{}", path.display());
    if let Ok(map) = cache().lock() {
        if let Some(found) = map.get(&cache_key) {
            return Some(found.clone());
        }
    }
    let bytes = std::fs::read(path).ok()?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    let thumb = decoded.thumbnail(max_edge, max_edge);
    let mut png = Vec::new();
    thumb
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    let image = Arc::new(Image::from_bytes(ImageFormat::Png, png));
    if let Ok(mut map) = cache().lock() {
        if map.len() > 256 {
            map.retain(|k, _| !k.starts_with("thumb:"));
        }
        map.insert(cache_key, image.clone());
    }
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_jlac_builtin_presets_are_valid_unique_pngs() {
        assert!(BUILTINS.len() >= 60);
        for (i, icon) in BUILTINS.iter().enumerate() {
            assert!(!icon.key.is_empty());
            assert!(!icon.label.is_empty());
            // Valid PNG magic header.
            assert_eq!(
                &icon.png[..8],
                b"\x89PNG\r\n\x1a\n",
                "preset {} is not a valid PNG",
                icon.key
            );
            for prev in &BUILTINS[..i] {
                assert_ne!(prev.key, icon.key, "duplicate preset {}", icon.key);
            }
        }
        // Spot-check classic JLac keys.
        for expected in [
            "default",
            "horizon",
            "jlac",
            "fabricmc",
            "grass",
            "creeper",
            "diamond",
            "fox",
            "bee",
            "modrinth",
            "netherstar",
            "grass_legacy",
            "infinity_legacy",
        ] {
            assert!(
                BUILTINS.iter().any(|b| b.key == expected),
                "missing preset {expected}"
            );
        }
    }

    #[test]
    fn imports_custom_image_and_preserves_pixel_art() {
        let dir = std::env::temp_dir().join(format!("fnh-icon-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let src = dir.join("my custom icon!.png");
        let mut raw = image::RgbaImage::new(16, 16);
        for p in raw.pixels_mut() {
            *p = image::Rgba([0x13, 0x12, 0x54, 0xff]);
        }
        raw.save(&src).unwrap();

        let out_dir = dir.join("icons");
        let key = import_custom_icon_into(&src, &out_dir).unwrap();
        assert!(key.starts_with(CUSTOM_PREFIX));
        let stem = key.strip_prefix(CUSTOM_PREFIX).unwrap();
        let saved = out_dir.join(format!("{stem}.png"));
        assert!(saved.is_file());
        let reloaded = image::open(&saved).unwrap();
        assert_eq!((reloaded.width(), reloaded.height()), (128, 128));

        // Importing the same name again picks a non-colliding suffix.
        let key2 = import_custom_icon_into(&src, &out_dir).unwrap();
        assert_ne!(key, key2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
