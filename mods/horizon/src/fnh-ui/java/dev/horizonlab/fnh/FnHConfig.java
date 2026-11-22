package dev.horizonlab.fnh;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Properties;
import java.util.stream.Stream;

import net.fabricmc.loader.api.FabricLoader;

public final class FnHConfig {
	public static String customBgFile = "";
	public static int bgDimPercent = 8;
	public static int motionBlurPercent = 100;
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

	public static List<Path> availableCustomBackgrounds() {
		Path dir = backgroundsDir();
		List<Path> out = new ArrayList<>();
		try (Stream<Path> s = Files.list(dir)) {
			s.filter(Files::isRegularFile)
				.filter(p -> {
					String n = p.getFileName().toString().toLowerCase(Locale.ROOT);
					return n.endsWith(".gif") || n.endsWith(".png") || n.endsWith(".jpg") || n.endsWith(".jpeg");
				})
				.sorted(Comparator.comparing(p -> p.getFileName().toString().toLowerCase(Locale.ROOT)))
				.forEach(out::add);
		} catch (IOException ignored) {
		}
		return out;
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
			motionBlurPercent = clamp(parseInt(props.getProperty("motionBlurPercent"), 100), 0, 150);
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
				props.store(out, "FnH 1.8 Menu UI Configuration");
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
