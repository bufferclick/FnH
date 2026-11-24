package dev.horizonlab.fnh;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Properties;
import java.util.stream.Stream;

import net.fabricmc.loader.api.FabricLoader;

public final class FnHConfig {
	public static final String DEFAULT_SENTINEL = "__default__";

	public static String customBgFile = "";
	public static int bgDimPercent = 8;
	public static int motionBlurPercent = 85;
	public static boolean showPlayerSkin = true;
	public static String skinOverrideUser = "";

	private FnHConfig() {
	}

	public static Path configFile() {
		return FabricLoader.getInstance().getConfigDir().resolve("fnh-menu.properties");
	}

	public static Path backgroundsDir() {
		Path dir = FabricLoader.getInstance().getConfigDir().resolve("fnh-backgrounds");
		try {
			Files.createDirectories(dir);
		} catch (IOException ignored) {
		}
		return dir;
	}

	public static List<Path> backgroundSearchDirs() {
		List<Path> dirs = new ArrayList<>();
		Path gameDir = FabricLoader.getInstance().getGameDir();
		dirs.add(backgroundsDir());
		dirs.add(gameDir.resolve("fnh-backgrounds"));
		dirs.add(gameDir.resolve("backgrounds"));
		String appData = System.getenv("APPDATA");
		if (appData != null && !appData.isBlank()) {
			dirs.add(Path.of(appData, "FnH", "backgrounds"));
		}
		return dirs;
	}

	private static boolean isSupportedImageName(String name) {
		String n = name.toLowerCase(Locale.ROOT);
		return n.endsWith(".png")
			|| n.endsWith(".jpg")
			|| n.endsWith(".jpeg")
			|| n.endsWith(".gif")
			|| n.endsWith(".bmp")
			|| n.endsWith(".webp");
	}

	public static List<Path> availableCustomBackgrounds() {
		Map<String, Path> unique = new LinkedHashMap<>();
		for (Path dir : backgroundSearchDirs()) {
			if (!Files.isDirectory(dir)) {
				continue;
			}
			try (Stream<Path> s = Files.list(dir)) {
				s.filter(Files::isRegularFile)
					.filter(p -> isSupportedImageName(p.getFileName().toString()))
					.sorted(Comparator.comparing(p -> p.getFileName().toString().toLowerCase(Locale.ROOT)))
					.forEach(p -> unique.putIfAbsent(p.getFileName().toString().toLowerCase(Locale.ROOT), p));
			} catch (IOException ignored) {
			}
		}
		return new ArrayList<>(unique.values());
	}

	public static Path resolveActiveBackgroundPath() {
		String pref = customBgFile == null ? "" : customBgFile.trim();
		if (DEFAULT_SENTINEL.equalsIgnoreCase(pref)) {
			return null;
		}
		List<Path> available = availableCustomBackgrounds();
		if (pref.isEmpty()) {
			return available.isEmpty() ? null : available.get(0);
		}
		for (Path p : available) {
			if (p.getFileName().toString().equalsIgnoreCase(pref) || p.toString().equalsIgnoreCase(pref)) {
				return p;
			}
		}
		try {
			Path direct = Path.of(pref);
			if (Files.isRegularFile(direct)) {
				return direct;
			}
		} catch (Exception ignored) {
		}
		return available.isEmpty() ? null : available.get(0);
	}

	public static void load() {
		backgroundsDir();
		Path file = configFile();
		if (!Files.isRegularFile(file)) {
			save();
			return;
		}
		Properties props = new Properties();
		try (InputStream in = Files.newInputStream(file)) {
			props.load(in);
			customBgFile = props.getProperty("customBgFile", "").trim();
			bgDimPercent = clamp(parseInt(props.getProperty("bgDimPercent"), 8), 0, 65);
			motionBlurPercent = clamp(parseInt(props.getProperty("motionBlurPercent"), 85), 0, 150);
			showPlayerSkin = Boolean.parseBoolean(props.getProperty("showPlayerSkin", "true"));
			skinOverrideUser = props.getProperty("skinOverrideUser", "").trim();
		} catch (IOException ignored) {
		}
	}

	public static void save() {
		Properties props = new Properties();
		props.setProperty("customBgFile", customBgFile == null ? "" : customBgFile);
		props.setProperty("bgDimPercent", Integer.toString(bgDimPercent));
		props.setProperty("motionBlurPercent", Integer.toString(motionBlurPercent));
		props.setProperty("showPlayerSkin", Boolean.toString(showPlayerSkin));
		props.setProperty("skinOverrideUser", skinOverrideUser == null ? "" : skinOverrideUser);
		Path file = configFile();
		try {
			Files.createDirectories(file.getParent());
			try (OutputStream out = Files.newOutputStream(file)) {
				props.store(out, "FnH 1.9.1 UI & Motion Blur Configuration");
			}
		} catch (IOException ignored) {
		}
	}

	private static int parseInt(String raw, int fallback) {
		if (raw == null) {
			return fallback;
		}
		try {
			return Integer.parseInt(raw.trim());
		} catch (NumberFormatException e) {
			return fallback;
		}
	}

	private static int clamp(int v, int lo, int hi) {
		return Math.max(lo, Math.min(hi, v));
	}
}
