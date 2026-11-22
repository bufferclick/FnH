package dev.horizonlab.fnh;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.Arrays;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import javax.imageio.ImageIO;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.util.Util;

public final class FnHSkinStageRenderer {
	private static final String BUFFERCLICK_UUID = "1b7a28177df149828231b48f5909ca95";
	private static final int BUF_SIZE = 176;

	private static final int[] SKIN_PIXELS = new int[64 * 64];
	private static final int[] COLOR_BUF = new int[BUF_SIZE * BUF_SIZE];
	private static final float[] DEPTH_BUF = new float[BUF_SIZE * BUF_SIZE];

	private static volatile boolean slimArms = false;
	private static String loadedSkinKey = null;
	private static boolean fetchInFlight = false;

	private static long introStartMs = 0L;
	private static long prevFrameMs = 0L;
	private static float dragYawOffset = 0.0F;
	private static float smoothAimYaw = 0.0F;
	private static float smoothAimPitch = 0.0F;

	static {
		initFallbackSkin();
	}

	private FnHSkinStageRenderer() {
	}

	public static void triggerWalkIn() {
		introStartMs = Util.getMillis();
		dragYawOffset = 0.0F;
	}

	public static void addDragYaw(float deltaRadians) {
		dragYawOffset += deltaRadians;
	}

	public static void forceReloadSkin() {
		loadedSkinKey = null;
	}

	public static void render(
		GuiGraphics graphics,
		int screenWidth,
		int stageCenterX,
		int stageTopY,
		int stageSizeGui,
		int mouseX,
		int mouseY
	) {
		if (!FnHConfig.showPlayerSkin || stageSizeGui < 24) {
			return;
		}
		ensureSkinLoaded();

		long now = Util.getMillis();
		if (introStartMs == 0L) {
			introStartMs = now;
			prevFrameMs = now;
		}
		float dt = Math.min(0.05F, Math.max(0.001F, (now - prevFrameMs) * 0.001F));
		prevFrameMs = now;

		float elapsed = Math.max(0.0F, (now - introStartMs) * 0.001F);
		float offscreenDist = screenWidth * 0.5F + stageSizeGui * 0.75F;
		float introDur = Math.max(1.9F, Math.min(2.6F, offscreenDist / 160.0F));
		float introP = Math.min(1.0F, elapsed / introDur);
		float offsetX = (1.0F - walkEase(introP)) * offscreenDist;

		float turnBlend = smoothstep(0.74F, 1.0F, introP);
		float strideEnv = 1.0F - smoothstep(0.76F, 1.0F, introP);
		float walkCycle = elapsed * 9.5F;
		float legSwing = (float) Math.sin(walkCycle) * 0.60F * strideEnv;
		float armWalk = (float) Math.sin(walkCycle) * 0.56F * strideEnv;
		float tSec = now * 0.001F;
		float posY = Math.abs((float) Math.sin(walkCycle)) * 0.45F * strideEnv
			+ (float) Math.sin(tSec * 2.0F) * 0.16F * turnBlend;

		float drawCenterX = stageCenterX + offsetX;
		float eyeX = drawCenterX;
		float eyeY = stageTopY + stageSizeGui * 0.24F;

		float targetYaw = clamp((float) Math.atan2(mouseX - eyeX, 130.0F), -1.05F, 1.05F);
		float targetPitch = clamp((float) Math.atan2(mouseY - eyeY, 130.0F), -0.76F, 0.80F);

		float damp = 1.0F - (float) Math.exp(-dt * 15.0F);
		smoothAimYaw += (targetYaw - smoothAimYaw) * damp;
		smoothAimPitch += (targetPitch - smoothAimPitch) * damp;

		float standBodyYaw = dragYawOffset + smoothAimYaw * 0.20F;
		float curBodyYaw = -1.48F * (1.0F - turnBlend) + standBodyYaw * turnBlend;
		float headLocalYaw = clamp((dragYawOffset + smoothAimYaw) - standBodyYaw, -0.92F, 0.92F) * turnBlend;
		float headPitch = smoothAimPitch * turnBlend;

		Arrays.fill(COLOR_BUF, 0);
		Arrays.fill(DEPTH_BUF, 1.0e9F);

		float[] vp = matMul(
			matPersp((float) Math.toRadians(26.0), 1.0F, 2.0F, 260.0F),
			matTranslate(0.0F, 1.9F, -84.0F)
		);
		float[] root = matMul(matTranslate(0.0F, posY, 0.0F), matRotY(curBodyYaw));
		float[] headMat = matMul(root, matMul(matTranslate(0.0F, 6.0F, 0.0F), matMul(matRotY(headLocalYaw), matRotX(headPitch))));

		boolean slim = slimArms;
		float aw = slim ? 3.0F : 4.0F;
		float armX = slim ? 5.65F : 6.18F;
		float idleSwing = (float) Math.sin(tSec * 1.5F) * 0.04F * turnBlend;
		float flap = 0.038F + (float) Math.cos(tSec * 1.3F) * 0.016F;

		float[] rArmMat = matMul(root, matMul(matTranslate(-armX, 4.0F, 0.0F), matMul(matRotZ(-flap), matRotX(armWalk + idleSwing))));
		float[] lArmMat = matMul(root, matMul(matTranslate(armX, 4.0F, 0.0F), matMul(matRotZ(flap), matRotX(-armWalk - idleSwing))));
		float[] rLegMat = matMul(root, matMul(matTranslate(-2.08F, -6.0F, 0.0F), matRotX(-legSwing)));
		float[] lLegMat = matMul(root, matMul(matTranslate(2.08F, -6.0F, 0.0F), matRotX(legSwing)));

		// Opaque inner body parts
		rasterBox(8, 12, 4, 16, 16, 0.0F, 0.0F, 0.0F, 0.0F, vp, root, false, true);
		rasterBox(8, 8, 8, 0, 0, 0.0F, 0.0F, 4.0F, 0.0F, vp, headMat, false, true);
		rasterBox(aw, 12, 4, 40, 16, 0.0F, 0.0F, -4.0F, 0.0F, vp, rArmMat, false, true);
		rasterBox(aw, 12, 4, 32, 48, 0.0F, 0.0F, -4.0F, 0.0F, vp, lArmMat, false, true);
		rasterBox(4, 12, 4, 0, 16, 0.0F, 0.0F, -6.0F, 0.0F, vp, rLegMat, false, true);
		rasterBox(4, 12, 4, 16, 48, 0.0F, 0.0F, -6.0F, 0.0F, vp, lLegMat, false, true);

		// Tiered outer skin layers (back faces then front faces)
		for (boolean frontPass : new boolean[] { false, true }) {
			rasterBox(4, 12, 4, 0, 32, 0.16F, 0.0F, -6.0F, 0.0F, vp, rLegMat, true, frontPass);
			rasterBox(4, 12, 4, 0, 48, 0.16F, 0.0F, -6.0F, 0.0F, vp, lLegMat, true, frontPass);
			rasterBox(aw, 12, 4, 40, 32, 0.22F, 0.0F, -4.0F, 0.0F, vp, rArmMat, true, frontPass);
			rasterBox(aw, 12, 4, 48, 48, 0.22F, 0.0F, -4.0F, 0.0F, vp, lArmMat, true, frontPass);
			rasterBox(8, 12, 4, 16, 32, 0.30F, 0.0F, 0.0F, 0.0F, vp, root, true, frontPass);
			rasterBox(8, 8, 8, 32, 0, 0.48F, 0.0F, 4.0F, 0.0F, vp, headMat, true, frontPass);
		}

		float scale = (float) stageSizeGui / (float) BUF_SIZE;
		float drawX = drawCenterX - stageSizeGui * 0.5F;

		// Subtle velocity ghost pass during brisk walk-in when motion blur is enabled
		if (introP < 0.85F && FnHConfig.motionBlurPercent > 15) {
			float trailOffset = (3.0F * (FnHConfig.motionBlurPercent / 100.0F));
			graphics.pose().pushMatrix();
			graphics.pose().translate(drawX + trailOffset, stageTopY);
			graphics.pose().scale(scale, scale);
			emitScanlines(graphics, 0x38);
			graphics.pose().popMatrix();
		}

		graphics.pose().pushMatrix();
		graphics.pose().translate(drawX, stageTopY);
		graphics.pose().scale(scale, scale);
		emitScanlines(graphics, 0xFF);
		graphics.pose().popMatrix();
	}

	private static void emitScanlines(GuiGraphics graphics, int alphaCap) {
		for (int y = 0; y < BUF_SIZE; y++) {
			int rowOff = y * BUF_SIZE;
			int x = 0;
			while (x < BUF_SIZE) {
				int c = COLOR_BUF[rowOff + x];
				if ((c >>> 24) == 0) {
					x++;
					continue;
				}
				int end = x + 1;
				while (end < BUF_SIZE && COLOR_BUF[rowOff + end] == c) {
					end++;
				}
				int drawCol = c;
				if (alphaCap < 0xFF) {
					int a = (((c >>> 24) & 0xFF) * alphaCap) / 255;
					drawCol = (a << 24) | (c & 0x00FFFFFF);
				}
				graphics.fill(x, y, end, y + 1, drawCol);
				x = end;
			}
		}
	}

	private static void rasterBox(
		float w, float h, float d,
		int u0, int v0, float inflate,
		float ox, float oy, float oz,
		float[] vp, float[] model,
		boolean outerLayer, boolean frontFaces
	) {
		float x0 = ox - w * 0.5F - inflate;
		float x1 = ox + w * 0.5F + inflate;
		float y0 = oy - h * 0.5F - inflate;
		float y1 = oy + h * 0.5F + inflate;
		float z0 = oz - d * 0.5F - inflate;
		float z1 = oz + d * 0.5F + inflate;

		int iw = Math.round(w);
		int ih = Math.round(h);
		int id = Math.round(d);

		float[] mvp = matMul(vp, model);

		// Front (+Z)
		rasterQuad(mvp, model, x0, y1, z1, x1, y1, z1, x1, y0, z1, x0, y0, z1,
			u0 + id, v0 + id, iw, ih, 0, 0, 1, outerLayer, frontFaces);
		// Back (-Z)
		rasterQuad(mvp, model, x1, y1, z0, x0, y1, z0, x0, y0, z0, x1, y0, z0,
			u0 + id + iw + id, v0 + id, iw, ih, 0, 0, -1, outerLayer, frontFaces);
		// Left (-X)
		rasterQuad(mvp, model, x0, y1, z0, x0, y1, z1, x0, y0, z1, x0, y0, z0,
			u0, v0 + id, id, ih, -1, 0, 0, outerLayer, frontFaces);
		// Right (+X)
		rasterQuad(mvp, model, x1, y1, z1, x1, y1, z0, x1, y0, z0, x1, y0, z1,
			u0 + id + iw, v0 + id, id, ih, 1, 0, 0, outerLayer, frontFaces);
		// Top (+Y)
		rasterQuad(mvp, model, x0, y1, z0, x1, y1, z0, x1, y1, z1, x0, y1, z1,
			u0 + id, v0, iw, id, 0, 1, 0, outerLayer, frontFaces);
		// Bottom (-Y)
		rasterQuad(mvp, model, x0, y0, z1, x1, y0, z1, x1, y0, z0, x0, y0, z0,
			u0 + id + iw, v0, iw, id, 0, -1, 0, outerLayer, frontFaces);
	}

	private static void rasterQuad(
		float[] mvp, float[] model,
		float ax, float ay, float az,
		float bx, float by, float bz,
		float cx, float cy, float cz,
		float dx, float dy, float dz,
		int u0, int v0, int uw, int vh,
		float nx, float ny, float nz,
		boolean outerLayer, boolean wantFront
	) {
		float wnx = model[0] * nx + model[4] * ny + model[8] * nz;
		float wny = model[1] * nx + model[5] * ny + model[9] * nz;
		float wnz = model[2] * nx + model[6] * ny + model[10] * nz;
		float len = (float) Math.sqrt(wnx * wnx + wny * wny + wnz * wnz);
		if (len > 1e-5F) {
			wnx /= len;
			wny /= len;
			wnz /= len;
		}
		float d1 = Math.max(0.0F, wnx * 0.289F + wny * 0.686F + wnz * 0.668F) * 0.40F;
		float d2 = Math.max(0.0F, wnx * -0.685F + wny * 0.342F + wnz * -0.642F) * 0.16F;
		float shade = Math.min(1.12F, 0.63F + d1 + d2);

		float[] pa = project(mvp, ax, ay, az);
		float[] pb = project(mvp, bx, by, bz);
		float[] pc = project(mvp, cx, cy, cz);
		float[] pd = project(mvp, dx, dy, dz);
		if (pa == null || pb == null || pc == null || pd == null) {
			return;
		}

		rasterTri(pa, pd, pc, u0, v0, u0, v0 + vh, u0 + uw, v0 + vh, shade, outerLayer, wantFront);
		rasterTri(pa, pc, pb, u0, v0, u0 + uw, v0 + vh, u0 + uw, v0, shade, outerLayer, wantFront);
	}

	private static float[] project(float[] mvp, float x, float y, float z) {
		float cx = mvp[0] * x + mvp[4] * y + mvp[8] * z + mvp[12];
		float cy = mvp[1] * x + mvp[5] * y + mvp[9] * z + mvp[13];
		float cz = mvp[2] * x + mvp[6] * y + mvp[10] * z + mvp[14];
		float cw = mvp[3] * x + mvp[7] * y + mvp[11] * z + mvp[15];
		if (cw <= 0.1F) {
			return null;
		}
		float invW = 1.0F / cw;
		float sx = (cx * invW * 0.5F + 0.5F) * BUF_SIZE;
		float sy = (0.5F - cy * invW * 0.5F) * BUF_SIZE;
		float sz = cz * invW;
		return new float[] { sx, sy, sz, invW };
	}

	private static void rasterTri(
		float[] p0, float[] p1, float[] p2,
		float u0, float v0, float u1, float v1, float u2, float v2,
		float shade, boolean outerLayer, boolean wantFront
	) {
		float area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
		if (Math.abs(area) < 1e-4F) {
			return;
		}
		boolean isFront = area < 0.0F;
		if (isFront != wantFront) {
			return;
		}

		int minX = Math.max(0, (int) Math.floor(Math.min(p0[0], Math.min(p1[0], p2[0]))));
		int maxX = Math.min(BUF_SIZE - 1, (int) Math.ceil(Math.max(p0[0], Math.max(p1[0], p2[0]))));
		int minY = Math.max(0, (int) Math.floor(Math.min(p0[1], Math.min(p1[1], p2[1]))));
		int maxY = Math.min(BUF_SIZE - 1, (int) Math.ceil(Math.max(p0[1], Math.max(p1[1], p2[1]))));

		float invArea = 1.0F / area;
		float zBias = outerLayer ? -0.0015F : 0.0F;

		for (int y = minY; y <= maxY; y++) {
			float py = y + 0.5F;
			for (int x = minX; x <= maxX; x++) {
				float px = x + 0.5F;
				float w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) * invArea;
				float w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) * invArea;
				float w2 = 1.0F - w0 - w1;
				if (w0 < 0.0F || w1 < 0.0F || w2 < 0.0F) {
					continue;
				}

				int idx = y * BUF_SIZE + x;
				float z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2] + zBias;
				if (z > DEPTH_BUF[idx]) {
					continue;
				}

				float invW = w0 * p0[3] + w1 * p1[3] + w2 * p2[3];
				float u = ((w0 * u0 * p0[3] + w1 * u1 * p1[3] + w2 * u2 * p2[3]) / invW);
				float v = ((w0 * v0 * p0[3] + w1 * v1 * p1[3] + w2 * v2 * p2[3]) / invW);
				int tx = clampInt((int) Math.floor(u), 0, 63);
				int ty = clampInt((int) Math.floor(v), 0, 63);

				int texel = SKIN_PIXELS[ty * 64 + tx];
				int ta = (texel >>> 24) & 0xFF;
				if (!outerLayer) {
					if (ta < 25) {
						continue;
					}
					ta = 255;
				} else if (ta < 10) {
					continue;
				}

				int tr = Math.min(255, Math.round(((texel >>> 16) & 0xFF) * shade));
				int tg = Math.min(255, Math.round(((texel >>> 8) & 0xFF) * shade));
				int tb = Math.min(255, Math.round((texel & 0xFF) * shade));

				if (ta >= 250 || (COLOR_BUF[idx] >>> 24) == 0) {
					COLOR_BUF[idx] = (ta << 24) | (tr << 16) | (tg << 8) | tb;
					if (!outerLayer || !wantFront) {
						DEPTH_BUF[idx] = z;
					}
				} else {
					int dst = COLOR_BUF[idx];
					int da = (dst >>> 24) & 0xFF;
					int dr = (dst >>> 16) & 0xFF;
					int dg = (dst >>> 8) & 0xFF;
					int db = dst & 0xFF;
					float sa = ta / 255.0F;
					int or = Math.min(255, Math.round(tr * sa + dr * (1.0F - sa)));
					int og = Math.min(255, Math.round(tg * sa + dg * (1.0F - sa)));
					int ob = Math.min(255, Math.round(tb * sa + db * (1.0F - sa)));
					int oa = Math.min(255, ta + Math.round(da * (1.0F - sa)));
					COLOR_BUF[idx] = (oa << 24) | (or << 16) | (og << 8) | ob;
				}
			}
		}
	}

	private static void ensureSkinLoaded() {
		String wanted = FnHConfig.skinOverrideUser == null ? "" : FnHConfig.skinOverrideUser.trim();
		String key;
		String queryTarget;
		if (!wanted.isEmpty()) {
			key = "override:" + wanted.toLowerCase(Locale.ROOT);
			queryTarget = wanted.equalsIgnoreCase("bufferclick") ? BUFFERCLICK_UUID : wanted;
		} else {
			Minecraft mc = Minecraft.getInstance();
			UUID uuid = mc.getUser().getProfileId();
			String name = mc.getUser().getName();
			if (uuid != null && !"00000000-0000-0000-0000-000000000000".equals(uuid.toString())) {
				key = "uuid:" + uuid;
				queryTarget = uuid.toString().replace("-", "");
			} else if (name != null && !name.isBlank()) {
				key = "name:" + name.toLowerCase(Locale.ROOT);
				queryTarget = name.equalsIgnoreCase("bufferclick") ? BUFFERCLICK_UUID : name;
			} else {
				key = "default:bufferclick";
				queryTarget = BUFFERCLICK_UUID;
			}
		}

		if (key.equals(loadedSkinKey) || fetchInFlight) {
			return;
		}
		loadedSkinKey = key;
		fetchInFlight = true;

		final String target = queryTarget;
		CompletableFuture.runAsync(() -> {
			try {
				HttpClient client = HttpClient.newBuilder()
					.connectTimeout(Duration.ofSeconds(4))
					.followRedirects(HttpClient.Redirect.NORMAL)
					.build();
				String[] urls = new String[] {
					"https://mc-heads.net/skin/" + target,
					"https://crafthead.net/skin/" + target,
					"https://minotar.net/skin/" + target
				};
				for (String u : urls) {
					try {
						HttpRequest req = HttpRequest.newBuilder(URI.create(u))
							.timeout(Duration.ofSeconds(5))
							.header("User-Agent", "FnH-Menu-UI/1.8")
							.GET()
							.build();
						HttpResponse<byte[]> resp = client.send(req, HttpResponse.BodyHandlers.ofByteArray());
						if (resp.statusCode() == 200 && resp.body() != null && resp.body().length > 64) {
							BufferedImage img = ImageIO.read(new ByteArrayInputStream(resp.body()));
							if (img != null && img.getWidth() >= 64) {
								ingestSkin(img);
								break;
							}
						}
					} catch (Exception ignored) {
					}
				}
			} finally {
				fetchInFlight = false;
			}
		});
	}

	private static void ingestSkin(BufferedImage img) {
		int[] next = new int[64 * 64];
		int h = Math.min(64, img.getHeight());
		img.getRGB(0, 0, 64, h, next, 0, 64);
		if (img.getHeight() == 32) {
			copyBlock(next, 0, 16, 16, 48, 16, 16);
			copyBlock(next, 40, 16, 32, 48, 16, 16);
		}
		slimArms = ((next[20 * 64 + 54] >>> 24) & 0xFF) == 0;
		System.arraycopy(next, 0, SKIN_PIXELS, 0, SKIN_PIXELS.length);
	}

	private static void copyBlock(int[] px, int sx, int sy, int dx, int dy, int w, int h) {
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				px[(dy + y) * 64 + (dx + x)] = px[(sy + y) * 64 + (sx + x)];
			}
		}
	}

	private static void initFallbackSkin() {
		fillSkinRect(0, 0, 32, 16, 0xFFE8BE96);
		fillSkinRect(8, 0, 8, 8, 0xFF1B1C22);
		fillSkinRect(8, 8, 8, 2, 0xFF1B1C22);
		fillSkinRect(0, 8, 8, 4, 0xFF1B1C22);
		fillSkinRect(16, 8, 16, 5, 0xFF1B1C22);
		fillSkinRect(9, 12, 2, 1, 0xFFFFFFFF);
		fillSkinRect(13, 12, 2, 1, 0xFFFFFFFF);
		fillSkinRect(10, 12, 1, 1, 0xFF14161F);
		fillSkinRect(13, 12, 1, 1, 0xFF14161F);
		fillSkinRect(16, 16, 24, 16, 0xFF17181D);
		fillSkinRect(22, 20, 4, 4, 0xFFD8DDE6);
		fillSkinRect(40, 16, 16, 16, 0xFF1D1F26);
		fillSkinRect(32, 48, 16, 16, 0xFF1D1F26);
		fillSkinRect(0, 16, 16, 16, 0xFF111216);
		fillSkinRect(16, 48, 16, 16, 0xFF111216);
	}

	private static void fillSkinRect(int x0, int y0, int w, int h, int argb) {
		for (int y = y0; y < y0 + h; y++) {
			for (int x = x0; x < x0 + w; x++) {
				SKIN_PIXELS[y * 64 + x] = argb;
			}
		}
	}

	private static float smoothstep(float a, float b, float x) {
		float t = clamp((x - a) / (b - a), 0.0F, 1.0F);
		return t * t * (3.0F - 2.0F * t);
	}

	private static float walkEase(float x) {
		float t = clamp(x, 0.0F, 1.0F);
		if (t < 0.72F) {
			return (t / 0.72F) * 0.82F;
		}
		float u = (t - 0.72F) / 0.28F;
		return 0.82F + 0.18F * (1.0F - (float) Math.pow(1.0F - u, 2.6));
	}

	private static float clamp(float v, float lo, float hi) {
		return Math.max(lo, Math.min(hi, v));
	}

	private static int clampInt(int v, int lo, int hi) {
		return Math.max(lo, Math.min(hi, v));
	}

	private static float[] matMul(float[] a, float[] b) {
		float[] o = new float[16];
		for (int c = 0; c < 4; c++) {
			for (int r = 0; r < 4; r++) {
				o[c * 4 + r] = a[r] * b[c * 4]
					+ a[4 + r] * b[c * 4 + 1]
					+ a[8 + r] * b[c * 4 + 2]
					+ a[12 + r] * b[c * 4 + 3];
			}
		}
		return o;
	}

	private static float[] matPersp(float fov, float asp, float n, float f) {
		float t = 1.0F / (float) Math.tan(fov * 0.5F);
		float nf = 1.0F / (n - f);
		return new float[] {
			t / asp, 0, 0, 0,
			0, t, 0, 0,
			0, 0, (f + n) * nf, -1,
			0, 0, (2 * f * n) * nf, 0
		};
	}

	private static float[] matTranslate(float x, float y, float z) {
		return new float[] {
			1, 0, 0, 0,
			0, 1, 0, 0,
			0, 0, 1, 0,
			x, y, z, 1
		};
	}

	private static float[] matRotX(float a) {
		float c = (float) Math.cos(a), s = (float) Math.sin(a);
		return new float[] {
			1, 0, 0, 0,
			0, c, s, 0,
			0, -s, c, 0,
			0, 0, 0, 1
		};
	}

	private static float[] matRotY(float a) {
		float c = (float) Math.cos(a), s = (float) Math.sin(a);
		return new float[] {
			c, 0, -s, 0,
			0, 1, 0, 0,
			s, 0, c, 0,
			0, 0, 0, 1
		};
	}

	private static float[] matRotZ(float a) {
		float c = (float) Math.cos(a), s = (float) Math.sin(a);
		return new float[] {
			c, s, 0, 0,
			-s, c, 0, 0,
			0, 0, 1, 0,
			0, 0, 0, 1
		};
	}
}
