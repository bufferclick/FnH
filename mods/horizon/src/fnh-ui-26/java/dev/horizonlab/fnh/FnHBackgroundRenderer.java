package dev.horizonlab.fnh;

import java.awt.Graphics2D;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import javax.imageio.ImageIO;
import javax.imageio.ImageReader;
import javax.imageio.metadata.IIOMetadata;
import javax.imageio.stream.ImageInputStream;

import com.mojang.blaze3d.platform.NativeImage;
import org.w3c.dom.NamedNodeMap;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.Util;

public final class FnHBackgroundRenderer {
	private static final String DEFAULT_GIF_RESOURCE = "/assets/fnh_ui/textures/gui/milkyway_bg.gif";
	private static final String LOGO_RESOURCE = "/assets/fnh_ui/textures/gui/minecraft_logo.png";
	private static final String EDITION_RESOURCE = "/assets/fnh_ui/textures/gui/fnh_edition.png";

	private static final class FrameEntry {
		final Identifier id;
		final int width;
		final int height;
		final int delayMs;

		FrameEntry(Identifier id, int width, int height, int delayMs) {
			this.id = id;
			this.width = width;
			this.height = height;
			this.delayMs = delayMs;
		}
	}

	private static final List<FrameEntry> FRAMES = new ArrayList<>();
	private static int totalCycleMs = 1000;
	private static String loadedSignature = null;
	private static long lastFolderPollMs = 0L;
	private static int generationCounter = 0;

	private static Identifier registeredLogoId = null;
	private static Identifier registeredEditionId = null;

	private FnHBackgroundRenderer() {
	}

	public static void reload() {
		loadedSignature = null;
		lastFolderPollMs = 0L;
	}

	public static Identifier logoTexture() {
		if (registeredLogoId == null) {
			registeredLogoId = registerClasspathPng(LOGO_RESOURCE, "fnh_mc_logo");
		}
		return registeredLogoId != null
			? registeredLogoId
			: Identifier.withDefaultNamespace("textures/gui/title/minecraft.png");
	}

	public static Identifier editionTexture() {
		if (registeredEditionId == null) {
			registeredEditionId = registerClasspathPng(EDITION_RESOURCE, "fnh_edition_banner");
		}
		return registeredEditionId != null
			? registeredEditionId
			: Identifier.withDefaultNamespace("textures/gui/title/edition.png");
	}

	private static Identifier registerClasspathPng(String classpathPath, String name) {
		try (InputStream in = FnHBackgroundRenderer.class.getResourceAsStream(classpathPath)) {
			if (in == null) {
				return null;
			}
			NativeImage img = NativeImage.read(in);
			Identifier id = Identifier.fromNamespaceAndPath(FnHMenuMod.MOD_ID, "dynamic/" + name);
			DynamicTexture tex = new DynamicTexture(() -> FnHMenuMod.MOD_ID + "_" + name, img);
			Minecraft.getInstance().getTextureManager().register(id, tex);
			return id;
		} catch (Exception ignored) {
			return null;
		}
	}

	public static void render(GuiGraphicsExtractor graphics, int width, int height, boolean applyTitleDim) {
		long nowMs = Util.getMillis();
		ensureLoaded(nowMs);
		if (!FRAMES.isEmpty()) {
			FrameEntry frame = currentFrame(nowMs);
			blitCover(graphics, frame, width, height);
		} else {
			graphics.fill(0, 0, width, height, 0xFF050712);
		}

		if (applyTitleDim && FnHConfig.bgDimPercent > 0) {
			int alpha = Math.max(0, Math.min(220, Math.round(FnHConfig.bgDimPercent * 2.55F)));
			graphics.fill(0, 0, width, height, (alpha << 24) | 0x04060E);
		}
	}

	private static void blitCover(GuiGraphicsExtractor graphics, FrameEntry frame, int boxW, int boxH) {
		if (frame.width <= 0 || frame.height <= 0 || boxW <= 0 || boxH <= 0) {
			return;
		}
		float scale = Math.max((float) boxW / (float) frame.width, (float) boxH / (float) frame.height);
		float drawW = frame.width * scale;
		float drawH = frame.height * scale;
		float drawX = (boxW - drawW) * 0.5F;
		float drawY = (boxH - drawH) * 0.5F;

		graphics.pose().pushMatrix();
		graphics.pose().translate(drawX, drawY);
		graphics.pose().scale(scale, scale);
		graphics.blit(
			RenderPipelines.GUI_TEXTURED,
			frame.id,
			0,
			0,
			0.0F,
			0.0F,
			frame.width,
			frame.height,
			frame.width,
			frame.height,
			ARGB.white(1.0F)
		);
		graphics.pose().popMatrix();
	}

	private static FrameEntry currentFrame(long nowMs) {
		if (FRAMES.size() == 1 || totalCycleMs <= 0) {
			return FRAMES.get(0);
		}
		long t = nowMs % totalCycleMs;
		int acc = 0;
		for (FrameEntry f : FRAMES) {
			acc += f.delayMs;
			if (t < acc) {
				return f;
			}
		}
		return FRAMES.get(FRAMES.size() - 1);
	}

	private static void releaseFrames() {
		if (FRAMES.isEmpty()) {
			return;
		}
		try {
			var tm = Minecraft.getInstance().getTextureManager();
			for (FrameEntry f : FRAMES) {
				tm.release(f.id);
			}
		} catch (Throwable ignored) {
		}
		FRAMES.clear();
		totalCycleMs = 0;
	}

	private static void ensureLoaded(long nowMs) {
		if (loadedSignature != null && !FRAMES.isEmpty() && (nowMs - lastFolderPollMs) < 1200L) {
			return;
		}
		lastFolderPollMs = nowMs;

		Path activeCustom = FnHConfig.resolveActiveBackgroundPath();
		String nextSig = "__default_milkyway__";
		if (activeCustom != null && Files.isRegularFile(activeCustom)) {
			long mtime = 0L;
			try {
				mtime = Files.getLastModifiedTime(activeCustom).toMillis();
			} catch (Exception ignored) {
			}
			nextSig = activeCustom.toAbsolutePath() + "|" + mtime;
		}
		if (nextSig.equals(loadedSignature) && !FRAMES.isEmpty()) {
			return;
		}
		loadedSignature = nextSig;
		releaseFrames();
		generationCounter++;

		boolean ok = false;
		if (activeCustom != null && Files.isRegularFile(activeCustom)) {
			try {
				byte[] bytes = Files.readAllBytes(activeCustom);
				boolean isGif = activeCustom.getFileName().toString().toLowerCase(Locale.ROOT).endsWith(".gif");
				ok = decodeBytes(activeCustom, bytes, isGif);
			} catch (Exception ignored) {
			}
		}

		if (!ok) {
			releaseFrames();
			try (InputStream in = FnHBackgroundRenderer.class.getResourceAsStream(DEFAULT_GIF_RESOURCE)) {
				if (in != null) {
					decodeBytes(null, in.readAllBytes(), true);
				}
			} catch (Exception ignored) {
			}
		}

		if (totalCycleMs <= 0) {
			totalCycleMs = Math.max(100, FRAMES.size() * 70);
		}
	}

	private static boolean decodeBytes(Path srcPath, byte[] bytes, boolean preferGif) throws Exception {
		if (bytes == null || bytes.length < 8) {
			return false;
		}

		if (preferGif) {
			try (ImageInputStream iis = ImageIO.createImageInputStream(new ByteArrayInputStream(bytes))) {
				Iterator<ImageReader> readers = ImageIO.getImageReadersByFormatName("gif");
				if (readers.hasNext()) {
					ImageReader reader = readers.next();
					try {
						reader.setInput(iis, false, false);
						int count = reader.getNumImages(true);
						if (count > 0) {
							BufferedImage first = reader.read(0);
							int canvasW = first.getWidth();
							int canvasH = first.getHeight();
							BufferedImage composite = new BufferedImage(canvasW, canvasH, BufferedImage.TYPE_INT_ARGB);

							int limit = Math.min(count, 72);
							for (int i = 0; i < limit; i++) {
								BufferedImage rawFrame = (i == 0) ? first : reader.read(i);
								int ox = 0;
								int oy = 0;
								int delayMs = 70;
								try {
									IIOMetadata meta = reader.getImageMetadata(i);
									String fmt = meta.getNativeMetadataFormatName();
									Node root = meta.getAsTree(fmt);
									delayMs = extractGifDelayMs(root, 70);
									int[] offset = extractGifOffset(root);
									ox = offset[0];
									oy = offset[1];
								} catch (Exception ignored) {
								}

								Graphics2D g2 = composite.createGraphics();
								g2.drawImage(rawFrame, ox, oy, null);
								g2.dispose();

								registerNativeFrame(bufferedImageToNative(composite), i, delayMs);
							}
							return !FRAMES.isEmpty();
						}
					} finally {
						reader.dispose();
					}
				}
			} catch (Exception ignored) {
			}
		}

		if ((bytes[0] & 0xFF) == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47) {
			try {
				NativeImage png = NativeImage.read(new ByteArrayInputStream(bytes));
				registerNativeFrame(png, 0, 1000);
				return true;
			} catch (Throwable ignored) {
			}
		}

		try {
			BufferedImage still = ImageIO.read(new ByteArrayInputStream(bytes));
			if (still != null && still.getWidth() > 0 && still.getHeight() > 0) {
				registerNativeFrame(bufferedImageToNative(still), 0, 1000);
				return true;
			}
		} catch (Throwable ignored) {
		}

		if (srcPath != null) {
			Path convertedPng = convertViaWindowsImaging(srcPath);
			if (convertedPng != null && Files.isRegularFile(convertedPng)) {
				NativeImage png = NativeImage.read(new ByteArrayInputStream(Files.readAllBytes(convertedPng)));
				registerNativeFrame(png, 0, 1000);
				return true;
			}
		}

		return false;
	}

	private static NativeImage bufferedImageToNative(BufferedImage src) {
		int w = src.getWidth();
		int h = src.getHeight();
		BufferedImage argbImg = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		Graphics2D g = argbImg.createGraphics();
		g.drawImage(src, 0, 0, null);
		g.dispose();

		int[] pixels = new int[w * h];
		argbImg.getRGB(0, 0, w, h, pixels, 0, w);
		NativeImage out = new NativeImage(w, h, false);
		for (int y = 0; y < h; y++) {
			int row = y * w;
			for (int x = 0; x < w; x++) {
				out.setPixel(x, y, pixels[row + x]);
			}
		}
		return out;
	}

	private static Path convertViaWindowsImaging(Path srcPath) {
		String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
		if (!os.contains("win")) {
			return null;
		}
		try {
			Path cacheDir = FnHConfig.backgroundsDir().resolve(".cache");
			Files.createDirectories(cacheDir);
			Path outPng = cacheDir.resolve(srcPath.getFileName().toString() + ".png");
			if (Files.isRegularFile(outPng)
				&& Files.getLastModifiedTime(outPng).toMillis() >= Files.getLastModifiedTime(srcPath).toMillis()) {
				return outPng;
			}
			String psScript =
				"Add-Type -AssemblyName PresentationCore; "
				+ "$s = [System.IO.File]::OpenRead('" + srcPath.toAbsolutePath().toString().replace("'", "''") + "'); "
				+ "$d = [System.Windows.Media.Imaging.BitmapDecoder]::Create($s, 'PreservePixelFormat', 'OnLoad'); "
				+ "$s.Close(); "
				+ "$e = New-Object System.Windows.Media.Imaging.PngBitmapEncoder; "
				+ "$e.Frames.Add($d.Frames[0]); "
				+ "$o = [System.IO.File]::Create('" + outPng.toAbsolutePath().toString().replace("'", "''") + "'); "
				+ "$e.Save($o); $o.Close();";
			Process proc = new ProcessBuilder("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", psScript)
				.start();
			proc.waitFor();
			if (Files.isRegularFile(outPng) && Files.size(outPng) > 64L) {
				return outPng;
			}
		} catch (Throwable ignored) {
		}
		return null;
	}

	private static void registerNativeFrame(NativeImage nativeImg, int index, int delayMs) {
		int w = nativeImg.getWidth();
		int h = nativeImg.getHeight();
		Identifier id = Identifier.fromNamespaceAndPath(
			FnHMenuMod.MOD_ID,
			"dynamic_bg_" + generationCounter + "_" + index
		);
		String label = FnHMenuMod.MOD_ID + "_bg_" + generationCounter + "_" + index;
		DynamicTexture tex = new DynamicTexture(() -> label, nativeImg);
		Minecraft.getInstance().getTextureManager().register(id, tex);

		int safeDelay = Math.max(25, delayMs);
		FRAMES.add(new FrameEntry(id, w, h, safeDelay));
		totalCycleMs += safeDelay;
	}

	private static int extractGifDelayMs(Node root, int fallbackMs) {
		NodeList children = root.getChildNodes();
		for (int i = 0; i < children.getLength(); i++) {
			Node n = children.item(i);
			if ("GraphicControlExtension".equals(n.getNodeName())) {
				NamedNodeMap attrs = n.getAttributes();
				if (attrs != null) {
					Node delayNode = attrs.getNamedItem("delayTime");
					if (delayNode != null) {
						try {
							int hundredths = Integer.parseInt(delayNode.getNodeValue());
							return hundredths <= 1 ? 70 : hundredths * 10;
						} catch (NumberFormatException ignored) {
						}
					}
				}
			}
		}
		return fallbackMs;
	}

	private static int[] extractGifOffset(Node root) {
		NodeList children = root.getChildNodes();
		for (int i = 0; i < children.getLength(); i++) {
			Node n = children.item(i);
			if ("ImageDescriptor".equals(n.getNodeName())) {
				NamedNodeMap attrs = n.getAttributes();
				if (attrs != null) {
					int left = parseAttrInt(attrs, "imageLeftPosition", 0);
					int top = parseAttrInt(attrs, "imageTopPosition", 0);
					return new int[] { left, top };
				}
			}
		}
		return new int[] { 0, 0 };
	}

	private static int parseAttrInt(NamedNodeMap attrs, String key, int def) {
		Node n = attrs.getNamedItem(key);
		if (n == null) {
			return def;
		}
		try {
			return Integer.parseInt(n.getNodeValue());
		} catch (NumberFormatException e) {
			return def;
		}
	}
}
