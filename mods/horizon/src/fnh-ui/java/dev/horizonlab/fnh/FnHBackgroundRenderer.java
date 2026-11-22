package dev.horizonlab.fnh;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import javax.imageio.ImageIO;
import javax.imageio.ImageReader;
import javax.imageio.metadata.IIOMetadata;
import javax.imageio.stream.ImageInputStream;

import com.mojang.blaze3d.platform.NativeImage;
import org.w3c.dom.NamedNodeMap;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.Util;

public final class FnHBackgroundRenderer {
	private static final String DEFAULT_GIF_RESOURCE = "/assets/fnh_ui/textures/gui/milkyway_bg.gif";
	private static final String LOGO_RESOURCE = "/assets/fnh_ui/textures/gui/minecraft_logo.png";
	private static final String EDITION_RESOURCE = "/assets/fnh_ui/textures/gui/fnh_edition.png";

	private static final List<Identifier> FRAME_IDS = new ArrayList<>();
	private static final List<Integer> FRAME_DELAYS_MS = new ArrayList<>();
	private static int totalCycleMs = 1000;
	private static String loadedSourceKey = null;
	private static int generationCounter = 0;

	private static Identifier registeredLogoId = null;
	private static Identifier registeredEditionId = null;

	private FnHBackgroundRenderer() {
	}

	public static void reload() {
		loadedSourceKey = null;
	}

	/**
	 * Registers `minecraft_logo.png` via `DynamicTexture` directly from classpath bytes so bare
	 * Fabric Loader (without Fabric API resource-loader) never shows a pink/black missing texture.
	 */
	public static Identifier logoTexture() {
		if (registeredLogoId == null) {
			registeredLogoId = registerClasspathPng(LOGO_RESOURCE, "fnh_mc_logo");
		}
		return registeredLogoId != null
			? registeredLogoId
			: Identifier.withDefaultNamespace("textures/gui/title/minecraft.png");
	}

	/**
	 * Registers `fnh_edition.png` (`512x64` `[FnH] EDITION`) via `DynamicTexture` from classpath bytes.
	 */
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

	public static void render(GuiGraphics graphics, int width, int height, boolean applyTitleDim) {
		ensureLoaded();
		if (!FRAME_IDS.isEmpty()) {
			Identifier tex = currentFrameTexture();
			graphics.blit(
				RenderPipelines.GUI_TEXTURED,
				tex,
				0,
				0,
				0.0F,
				0.0F,
				width,
				height,
				width,
				height,
				ARGB.white(1.0F)
			);
		} else {
			graphics.fill(0, 0, width, height, 0xFF050712);
		}

		if (applyTitleDim && FnHConfig.bgDimPercent > 0) {
			int alpha = Math.max(0, Math.min(220, Math.round(FnHConfig.bgDimPercent * 2.55F)));
			graphics.fill(0, 0, width, height, (alpha << 24) | 0x04060E);
		}
	}

	private static Identifier currentFrameTexture() {
		if (FRAME_IDS.size() == 1 || totalCycleMs <= 0) {
			return FRAME_IDS.get(0);
		}
		long t = Util.getMillis() % totalCycleMs;
		int acc = 0;
		for (int i = 0; i < FRAME_IDS.size(); i++) {
			acc += FRAME_DELAYS_MS.get(i);
			if (t < acc) {
				return FRAME_IDS.get(i);
			}
		}
		return FRAME_IDS.get(FRAME_IDS.size() - 1);
	}

	private static void ensureLoaded() {
		String wantedKey = FnHConfig.customBgFile == null ? "" : FnHConfig.customBgFile.trim();
		if (wantedKey.equals(loadedSourceKey) && !FRAME_IDS.isEmpty()) {
			return;
		}
		loadedSourceKey = wantedKey;
		FRAME_IDS.clear();
		FRAME_DELAYS_MS.clear();
		totalCycleMs = 0;
		generationCounter++;

		boolean ok = false;
		if (!wantedKey.isEmpty()) {
			Path candidate = FnHConfig.backgroundsDir().resolve(wantedKey);
			if (!Files.isRegularFile(candidate)) {
				Path direct = Path.of(wantedKey);
				if (Files.isRegularFile(direct)) {
					candidate = direct;
				}
			}
			if (Files.isRegularFile(candidate)) {
				try (InputStream in = Files.newInputStream(candidate)) {
					ok = decodeStream(in, candidate.getFileName().toString().toLowerCase().endsWith(".gif"));
				} catch (Exception ignored) {
				}
			}
		}

		if (!ok) {
			try (InputStream in = FnHBackgroundRenderer.class.getResourceAsStream(DEFAULT_GIF_RESOURCE)) {
				if (in != null) {
					decodeStream(in, true);
				}
			} catch (Exception ignored) {
			}
		}

		if (totalCycleMs <= 0) {
			totalCycleMs = Math.max(100, FRAME_IDS.size() * 70);
		}
	}

	private static boolean decodeStream(InputStream rawIn, boolean preferGif) throws Exception {
		byte[] bytes = rawIn.readAllBytes();
		if (bytes.length == 0) {
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

							int limit = Math.min(count, 64);
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

								java.awt.Graphics2D g2 = composite.createGraphics();
								g2.drawImage(rawFrame, ox, oy, null);
								g2.dispose();

								registerBufferedFrame(composite, i, delayMs);
							}
							return !FRAME_IDS.isEmpty();
						}
					} finally {
						reader.dispose();
					}
				}
			} catch (Exception ignored) {
			}
		}

		BufferedImage still = ImageIO.read(new ByteArrayInputStream(bytes));
		if (still == null) {
			return false;
		}
		registerBufferedFrame(still, 0, 1000);
		return !FRAME_IDS.isEmpty();
	}

	private static void registerBufferedFrame(BufferedImage img, int index, int delayMs) throws Exception {
		ByteArrayOutputStream pngBytes = new ByteArrayOutputStream(32768);
		ImageIO.write(img, "png", pngBytes);
		NativeImage nativeImg = NativeImage.read(new ByteArrayInputStream(pngBytes.toByteArray()));

		Identifier id = Identifier.fromNamespaceAndPath(
			FnHMenuMod.MOD_ID,
			"dynamic_bg_" + generationCounter + "_" + index
		);
		String label = FnHMenuMod.MOD_ID + "_bg_" + generationCounter + "_" + index;
		DynamicTexture tex = new DynamicTexture(() -> label, nativeImg);
		Minecraft.getInstance().getTextureManager().register(id, tex);

		int safeDelay = Math.max(25, delayMs);
		FRAME_IDS.add(id);
		FRAME_DELAYS_MS.add(safeDelay);
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
