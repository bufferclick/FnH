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
import net.minecraft.util.Util;

public final class FnHBackgroundRenderer {
	private static final String DEFAULT_GIF_RESOURCE = "/assets/fnh_ui/textures/gui/milkyway_bg.gif";

	private static final List<Identifier> FRAME_IDS = new ArrayList<>();
	private static final List<Integer> FRAME_DELAYS_MS = new ArrayList<>();
	private static int totalCycleMs = 1000;
	private static String loadedSourceKey = null;
	private static int generationCounter = 0;

	private FnHBackgroundRenderer() {
	}

	public static void reload() {
		loadedSourceKey = null;
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
				height
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

		if (preferGif || isGifMagic(bytes)) {
			Iterator<ImageReader> readers = ImageIO.getImageReadersByFormatName("gif");
			if (readers.hasNext()) {
				ImageReader reader = readers.next();
				try (ImageInputStream iis = ImageIO.createImageInputStream(new ByteArrayInputStream(bytes))) {
					reader.setInput(iis, false, false);
					int numFrames = reader.getNumImages(true);
					if (numFrames > 0) {
						int canvasW = 0;
						int canvasH = 0;
						BufferedImage first = reader.read(0);
						canvasW = first.getWidth();
						canvasH = first.getHeight();
						int[] master = new int[canvasW * canvasH];

						for (int i = 0; i < numFrames; i++) {
							BufferedImage frame = (i == 0) ? first : reader.read(i);
							int fx = 0;
							int fy = 0;
							int delayMs = 70;
							String disposal = "none";

							try {
								IIOMetadata meta = reader.getImageMetadata(i);
								Node root = meta.getAsTree("javax_imageio_gif_image_1.0");
								NodeList children = root.getChildNodes();
								for (int c = 0; c < children.getLength(); c++) {
									Node node = children.item(c);
									NamedNodeMap attrs = node.getAttributes();
									if (attrs == null) {
										continue;
									}
									if ("ImageDescriptor".equals(node.getNodeName())) {
										fx = attrInt(attrs, "imageLeftPosition", 0);
										fy = attrInt(attrs, "imageTopPosition", 0);
									} else if ("GraphicControlExtension".equals(node.getNodeName())) {
										int cs = attrInt(attrs, "delayTime", 7);
										delayMs = Math.max(20, cs * 10);
										Node dispNode = attrs.getNamedItem("disposalMethod");
										if (dispNode != null) {
											disposal = dispNode.getNodeValue();
										}
									}
								}
							} catch (Exception ignored) {
							}

							int[] backup = "restoreToPrevious".equals(disposal) ? master.clone() : null;
							int fw = frame.getWidth();
							int fh = frame.getHeight();
							int[] sub = frame.getRGB(0, 0, fw, fh, null, 0, fw);
							for (int y = 0; y < fh; y++) {
								int dy = fy + y;
								if (dy < 0 || dy >= canvasH) {
									continue;
								}
								for (int x = 0; x < fw; x++) {
									int dx = fx + x;
									if (dx < 0 || dx >= canvasW) {
										continue;
									}
									int argb = sub[y * fw + x];
									int a = (argb >>> 24) & 0xFF;
									if (a > 0) {
										master[dy * canvasW + dx] = argb | 0xFF000000;
									}
								}
							}

							BufferedImage composed = new BufferedImage(canvasW, canvasH, BufferedImage.TYPE_INT_ARGB);
							composed.setRGB(0, 0, canvasW, canvasH, master, 0, canvasW);
							registerBufferedFrame(composed, i, delayMs);

							if ("restoreToBackgroundColor".equals(disposal)) {
								for (int y = 0; y < fh; y++) {
									int dy = fy + y;
									if (dy < 0 || dy >= canvasH) {
										continue;
									}
									for (int x = 0; x < fw; x++) {
										int dx = fx + x;
										if (dx >= 0 && dx < canvasW) {
											master[dy * canvasW + dx] = 0xFF050712;
										}
									}
								}
							} else if (backup != null) {
								System.arraycopy(backup, 0, master, 0, master.length);
							}
						}
						return !FRAME_IDS.isEmpty();
					}
				} finally {
					reader.dispose();
				}
			}
		}

		BufferedImage single = ImageIO.read(new ByteArrayInputStream(bytes));
		if (single != null) {
			registerBufferedFrame(single, 0, 1000);
			return true;
		}
		return false;
	}

	private static void registerBufferedFrame(BufferedImage img, int idx, int delayMs) throws Exception {
		ByteArrayOutputStream baos = new ByteArrayOutputStream(32768);
		ImageIO.write(img, "png", baos);
		NativeImage nativeImg = NativeImage.read(new ByteArrayInputStream(baos.toByteArray()));
		String name = "bg_" + generationCounter + "_" + idx;
		Identifier id = Identifier.fromNamespaceAndPath(FnHMenuMod.MOD_ID, name);
		DynamicTexture tex = new DynamicTexture(() -> "fnh_" + name, nativeImg);
		Minecraft.getInstance().getTextureManager().register(id, tex);
		FRAME_IDS.add(id);
		FRAME_DELAYS_MS.add(delayMs);
		totalCycleMs += delayMs;
	}

	private static boolean isGifMagic(byte[] b) {
		return b.length >= 6 && b[0] == 'G' && b[1] == 'I' && b[2] == 'F';
	}

	private static int attrInt(NamedNodeMap attrs, String name, int def) {
		Node n = attrs.getNamedItem(name);
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
