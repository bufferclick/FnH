package dev.horizonlab.fnh;

import java.io.ByteArrayInputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Base64;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import com.mojang.blaze3d.platform.NativeImage;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.Util;

public final class FnHSkinStageRenderer {
	private static final String BUFFERCLICK_UUID = "1b7a28177df149828231b48f5909ca95";

	// 2x2 Supersampled Anti-Aliasing (512x512 subpixel raster -> 256x256 GPU DynamicTexture)
	private static final int OUT_SIZE = 256;
	private static final int SSAA = 2;
	private static final int BUF_SIZE = OUT_SIZE * SSAA;

	private static final int[] SKIN_PIXELS = new int[64 * 64];
	private static final int[] COLOR_BUF = new int[BUF_SIZE * BUF_SIZE];
	private static final float[] DEPTH_BUF = new float[BUF_SIZE * BUF_SIZE];

	private static DynamicTexture stageTexture = null;
	private static Identifier stageTextureId = null;
	private static int prevOutMinX = 0;
	private static int prevOutMaxX = OUT_SIZE - 1;
	private static int prevOutMinY = 0;
	private static int prevOutMaxY = OUT_SIZE - 1;
	private static int curSubMinX = BUF_SIZE;
	private static int curSubMaxX = 0;
	private static int curSubMinY = BUF_SIZE;
	private static int curSubMaxY = 0;

	private static final Pattern UUID_ID_PATTERN = Pattern.compile("\"id\"\\s*:\\s*\"([0-9a-fA-F]{32})\"");
	private static final Pattern TEXTURES_VALUE_PATTERN = Pattern.compile(
		"\"name\"\\s*:\\s*\"textures\"[\\s\\S]*?\"value\"\\s*:\\s*\"([A-Za-z0-9+/=_\\-]+)\""
	);
	private static final Pattern SKIN_BLOCK_PATTERN = Pattern.compile("\"SKIN\"\\s*:\\s*\\{([\\s\\S]*?)\\}(?:\\s*,\\s*\"|\\s*\\})");
	private static final Pattern SKIN_URL_PATTERN = Pattern.compile("\"url\"\\s*:\\s*\"([^\"]+)\"");
	private static final Pattern SLIM_MODEL_PATTERN = Pattern.compile("\"model\"\\s*:\\s*\"slim\"");

	private static volatile boolean slimArms = false;
	private static String loadedSkinKey = null;
	private static volatile boolean fetchInFlight = false;

	private static long introStartMs = 0L;
	private static long prevFrameMs = 0L;
	private static float dragYawOffset = 0.0F;
	private static float smoothAimYaw = 0.0F;
	private static float smoothAimPitch = 0.0F;

	static {
		initFallbackSkin();
		for (int i = 0; i < DEPTH_BUF.length; i++) {
			DEPTH_BUF[i] = 1.0e9F;
		}
	}

	private FnHSkinStageRenderer() {
	}

	public static void triggerWalkIn() {
		introStartMs = 0L;
		dragYawOffset = 0.0F;
	}

	public static void addDragYaw(float deltaRadians) {
		dragYawOffset += deltaRadians;
	}

	public static void forceReloadSkin() {
		loadedSkinKey = null;
	}

	private static void ensureStageTexture() {
		if (stageTexture != null && stageTextureId != null) {
			return;
		}
		stageTexture = new DynamicTexture(() -> FnHMenuMod.MOD_ID + "_skin_stage_3d", OUT_SIZE, OUT_SIZE, true);
		stageTextureId = Identifier.fromNamespaceAndPath(FnHMenuMod.MOD_ID, "dynamic/skin_stage_3d");
		Minecraft.getInstance().getTextureManager().register(stageTextureId, stageTexture);
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
		ensureStageTexture();

		Minecraft mc = Minecraft.getInstance();
		long now = Util.getMillis();
		// Hold the walk-in intro while Mojang's initial LoadingOverlay is still fading out on first boot!
		if (mc.getOverlay() != null) {
			introStartMs = 0L;
			prevFrameMs = now;
		} else if (introStartMs == 0L) {
			introStartMs = now;
			prevFrameMs = now;
		}
		float dt = Math.min(0.05F, Math.max(0.001F, (now - prevFrameMs) * 0.001F));
		prevFrameMs = now;

		float elapsed = introStartMs == 0L ? 0.0F : Math.max(0.0F, (now - introStartMs) * 0.001F);
		float offscreenDist = screenWidth * 0.5F + stageSizeGui * 0.75F;
		float introDur = Math.max(1.85F, Math.min(2.45F, offscreenDist / 165.0F));
		float introP = Math.min(1.0F, elapsed / introDur);
		float offsetX = (1.0F - walkEase(introP)) * offscreenDist;

		float turnBlend = smoothstep(0.74F, 1.0F, introP);
		float strideEnv = 1.0F - smoothstep(0.76F, 1.0F, introP);
		float walkCycle = elapsed * 9.5F;
		float legSwing = (float) Math.sin(walkCycle) * 0.58F * strideEnv;
		float armWalk = (float) Math.sin(walkCycle) * 0.54F * strideEnv;
		float tSec = now * 0.001F;
		float posY = Math.abs((float) Math.sin(walkCycle)) * 0.42F * strideEnv
			+ (float) Math.sin(tSec * 2.0F) * 0.14F * turnBlend;

		float drawCenterX = stageCenterX + offsetX;
		float eyeX = drawCenterX;
		float eyeY = stageTopY + stageSizeGui * 0.24F;
		float lookDist = Math.max(175.0F, stageSizeGui * 2.45F);

		float targetYaw = clamp((float) Math.atan2(mouseX - eyeX, lookDist), -0.55F, 0.55F);
		float targetPitch = clamp((float) Math.atan2(mouseY - eyeY, lookDist), -0.24F, 0.26F);

		float damp = 1.0F - (float) Math.exp(-dt * 14.0F);
		smoothAimYaw += (targetYaw - smoothAimYaw) * damp;
		smoothAimPitch += (targetPitch - smoothAimPitch) * damp;

		float standBodyYaw = dragYawOffset + smoothAimYaw * 0.22F;
		float curBodyYaw = -1.46F * (1.0F - turnBlend) + standBodyYaw * turnBlend;
		float headLocalYaw = clamp((dragYawOffset + smoothAimYaw) - standBodyYaw, -0.52F, 0.52F) * turnBlend;
		float headPitch = smoothAimPitch * turnBlend;

		// Clear only the previous frame's active subpixel bounding box
		int clrX0 = Math.max(0, prevOutMinX * SSAA);
		int clrX1 = Math.min(BUF_SIZE - 1, prevOutMaxX * SSAA + (SSAA - 1));
		int clrY0 = Math.max(0, prevOutMinY * SSAA);
		int clrY1 = Math.min(BUF_SIZE - 1, prevOutMaxY * SSAA + (SSAA - 1));
		for (int sy = clrY0; sy <= clrY1; sy++) {
			int row = sy * BUF_SIZE;
			for (int sx = clrX0; sx <= clrX1; sx++) {
				COLOR_BUF[row + sx] = 0;
				DEPTH_BUF[row + sx] = 1.0e9F;
			}
		}
		curSubMinX = BUF_SIZE;
		curSubMaxX = 0;
		curSubMinY = BUF_SIZE;
		curSubMaxY = 0;

		float[] vp = matMul(
			matPersp((float) Math.toRadians(25.5), 1.0F, 2.0F, 260.0F),
			matMul(matTranslate(0.0F, 1.85F, -84.0F), matRotX(0.06F))
		);
		float[] root = matMul(matTranslate(0.0F, posY, 0.0F), matRotY(curBodyYaw));
		float[] headMat = matMul(root, matMul(matTranslate(0.0F, 6.0F, 0.0F), matMul(matRotY(headLocalYaw), matRotX(headPitch))));

		boolean slim = slimArms;
		float aw = slim ? 3.0F : 4.0F;
		float armX = slim ? 5.62F : 6.14F;
		float idleSwing = (float) Math.sin(tSec * 1.5F) * 0.038F * turnBlend;
		float flap = 0.036F + (float) Math.cos(tSec * 1.3F) * 0.014F;

		float[] rArmMat = matMul(root, matMul(matTranslate(-armX, 4.0F, 0.0F), matMul(matRotZ(-flap), matRotX(armWalk + idleSwing))));
		float[] lArmMat = matMul(root, matMul(matTranslate(armX, 4.0F, 0.0F), matMul(matRotZ(flap), matRotX(-armWalk - idleSwing))));
		float[] rLegMat = matMul(root, matMul(matTranslate(-2.06F, -6.0F, 0.0F), matRotX(-legSwing)));
		float[] lLegMat = matMul(root, matMul(matTranslate(2.06F, -6.0F, 0.0F), matRotX(legSwing)));

		// Opaque inner body parts
		rasterBox(8, 12, 4, 16, 16, 0.0F, 0.0F, 0.0F, 0.0F, vp, root, false, true);
		rasterBox(8, 8, 8, 0, 0, 0.0F, 0.0F, 4.0F, 0.0F, vp, headMat, false, true);
		rasterBox(aw, 12, 4, 40, 16, 0.0F, 0.0F, -4.0F, 0.0F, vp, rArmMat, false, true);
		rasterBox(aw, 12, 4, 32, 48, 0.0F, 0.0F, -4.0F, 0.0F, vp, lArmMat, false, true);
		rasterBox(4, 12, 4, 0, 16, 0.0F, 0.0F, -6.0F, 0.0F, vp, rLegMat, false, true);
		rasterBox(4, 12, 4, 16, 48, 0.0F, 0.0F, -6.0F, 0.0F, vp, lLegMat, false, true);

		// Tiered outer 2nd-layer skin (hat, jacket, sleeves, pants — back faces then front faces)
		for (boolean frontPass : new boolean[] { false, true }) {
			rasterBox(4, 12, 4, 0, 32, 0.20F, 0.0F, -6.0F, 0.0F, vp, rLegMat, true, frontPass);
			rasterBox(4, 12, 4, 0, 48, 0.20F, 0.0F, -6.0F, 0.0F, vp, lLegMat, true, frontPass);
			rasterBox(aw, 12, 4, 40, 32, 0.24F, 0.0F, -4.0F, 0.0F, vp, rArmMat, true, frontPass);
			rasterBox(aw, 12, 4, 48, 48, 0.24F, 0.0F, -4.0F, 0.0F, vp, lArmMat, true, frontPass);
			rasterBox(8, 12, 4, 16, 32, 0.28F, 0.0F, 0.0F, 0.0F, vp, root, true, frontPass);
			rasterBox(8, 8, 8, 32, 0, 0.48F, 0.0F, 4.0F, 0.0F, vp, headMat, true, frontPass);
		}

		NativeImage outImg = stageTexture.getPixels();
		if (outImg == null) {
			return;
		}

		int nextOutMinX = curSubMinX <= curSubMaxX ? Math.max(0, curSubMinX / SSAA) : 0;
		int nextOutMaxX = curSubMinX <= curSubMaxX ? Math.min(OUT_SIZE - 1, curSubMaxX / SSAA) : 0;
		int nextOutMinY = curSubMinY <= curSubMaxY ? Math.max(0, curSubMinY / SSAA) : 0;
		int nextOutMaxY = curSubMinY <= curSubMaxY ? Math.min(OUT_SIZE - 1, curSubMaxY / SSAA) : 0;

		int updX0 = Math.min(prevOutMinX, nextOutMinX);
		int updX1 = Math.max(prevOutMaxX, nextOutMaxX);
		int updY0 = Math.min(prevOutMinY, nextOutMinY);
		int updY1 = Math.max(prevOutMaxY, nextOutMaxY);

		// Downsample 2x2 premultiplied alpha into crisp anti-aliased `NativeImage` pixels
		for (int oy = updY0; oy <= updY1; oy++) {
			int r0 = (oy * 2) * BUF_SIZE;
			int r1 = (oy * 2 + 1) * BUF_SIZE;
			for (int ox = updX0; ox <= updX1; ox++) {
				int sx = ox * 2;
				int c00 = COLOR_BUF[r0 + sx];
				int c10 = COLOR_BUF[r0 + sx + 1];
				int c01 = COLOR_BUF[r1 + sx];
				int c11 = COLOR_BUF[r1 + sx + 1];
				int aSum = (c00 >>> 24) + (c10 >>> 24) + (c01 >>> 24) + (c11 >>> 24);
				if (aSum == 0) {
					outImg.setPixel(ox, oy, 0);
				} else if (aSum == 1020 && c00 == c10 && c00 == c01 && c00 == c11) {
					outImg.setPixel(ox, oy, c00);
				} else {
					int a00 = c00 >>> 24, a10 = c10 >>> 24, a01 = c01 >>> 24, a11 = c11 >>> 24;
					int pr = ((c00 >> 16) & 0xFF) * a00 + ((c10 >> 16) & 0xFF) * a10
						+ ((c01 >> 16) & 0xFF) * a01 + ((c11 >> 16) & 0xFF) * a11;
					int pg = ((c00 >> 8) & 0xFF) * a00 + ((c10 >> 8) & 0xFF) * a10
						+ ((c01 >> 8) & 0xFF) * a01 + ((c11 >> 8) & 0xFF) * a11;
					int pb = (c00 & 0xFF) * a00 + (c10 & 0xFF) * a10
						+ (c01 & 0xFF) * a01 + (c11 & 0xFF) * a11;
					int outA = (aSum + 2) >> 2;
					int outR = Math.min(255, (pr + (aSum >> 1)) / aSum);
					int outG = Math.min(255, (pg + (aSum >> 1)) / aSum);
					int outB = Math.min(255, (pb + (aSum >> 1)) / aSum);
					outImg.setPixel(ox, oy, (outA << 24) | (outR << 16) | (outG << 8) | outB);
				}
			}
		}
		prevOutMinX = nextOutMinX;
		prevOutMaxX = nextOutMaxX;
		prevOutMinY = nextOutMinY;
		prevOutMaxY = nextOutMaxY;
		stageTexture.upload();

		float drawX = drawCenterX - stageSizeGui * 0.5F;

		// Smooth multi-tap walk-in motion blur when enabled
		if (introP < 0.84F && FnHConfig.motionBlurPercent > 15) {
			float blurScale = FnHConfig.motionBlurPercent / 100.0F;
			float speed = 1.0F - smoothstep(0.45F, 0.84F, introP);
			for (int tap = 2; tap >= 1; tap--) {
				float trailDx = tap * 2.4F * blurScale * speed;
				float tapAlpha = (tap == 1 ? 0.24F : 0.12F) * speed;
				graphics.pose().pushMatrix();
				graphics.pose().translate(drawX + trailDx, stageTopY);
				graphics.blit(
					RenderPipelines.GUI_TEXTURED,
					stageTextureId,
					0,
					0,
					0.0F,
					0.0F,
					stageSizeGui,
					stageSizeGui,
					OUT_SIZE,
					OUT_SIZE,
					ARGB.white(tapAlpha)
				);
				graphics.pose().popMatrix();
			}
		}

		graphics.pose().pushMatrix();
		graphics.pose().translate(drawX, stageTopY);
		graphics.blit(
			RenderPipelines.GUI_TEXTURED,
			stageTextureId,
			0,
			0,
			0.0F,
			0.0F,
			stageSizeGui,
			stageSizeGui,
			OUT_SIZE,
			OUT_SIZE,
			ARGB.white(1.0F)
		);
		graphics.pose().popMatrix();
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

		rasterQuad(mvp, model, x0, y1, z1, x1, y1, z1, x1, y0, z1, x0, y0, z1,
			u0 + id, v0 + id, iw, ih, 0, 0, 1, outerLayer, frontFaces);
		rasterQuad(mvp, model, x1, y1, z0, x0, y1, z0, x0, y0, z0, x1, y0, z0,
			u0 + id + iw + id, v0 + id, iw, ih, 0, 0, -1, outerLayer, frontFaces);
		rasterQuad(mvp, model, x0, y1, z0, x0, y1, z1, x0, y0, z1, x0, y0, z0,
			u0, v0 + id, id, ih, -1, 0, 0, outerLayer, frontFaces);
		rasterQuad(mvp, model, x1, y1, z1, x1, y1, z0, x1, y0, z0, x1, y0, z1,
			u0 + id + iw, v0 + id, id, ih, 1, 0, 0, outerLayer, frontFaces);
		rasterQuad(mvp, model, x0, y1, z0, x1, y1, z0, x1, y1, z1, x0, y1, z1,
			u0 + id, v0, iw, id, 0, 1, 0, outerLayer, frontFaces);
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
		float d1 = Math.max(0.0F, wnx * 0.30F + wny * 0.68F + wnz * 0.66F) * 0.42F;
		float d2 = Math.max(0.0F, wnx * -0.68F + wny * 0.34F + wnz * -0.64F) * 0.16F;
		float shade = Math.min(1.12F, 0.62F + d1 + d2);

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
		if (minX > maxX || minY > maxY) {
			return;
		}
		if (minX < curSubMinX) curSubMinX = minX;
		if (maxX > curSubMaxX) curSubMaxX = maxX;
		if (minY < curSubMinY) curSubMinY = minY;
		if (maxY > curSubMaxY) curSubMaxY = maxY;

		float invArea = 1.0F / area;
		float uw0 = u0 * p0[3], vw0 = v0 * p0[3];
		float uw1 = u1 * p1[3], vw1 = v1 * p1[3];
		float uw2 = u2 * p2[3], vw2 = v2 * p2[3];

		int uMin = (int) Math.floor(Math.min(u0, Math.min(u1, u2)));
		int uMax = (int) Math.ceil(Math.max(u0, Math.max(u1, u2))) - 1;
		int vMin = (int) Math.floor(Math.min(v0, Math.min(v1, v2)));
		int vMax = (int) Math.ceil(Math.max(v0, Math.max(v1, v2))) - 1;

		// Subtle 3D rim lift so pure #000000 black outfits retain crisp limb/torso separation
		int rimLift = Math.round(Math.max(0.0F, shade - 0.72F) * 34.0F);

		for (int py = minY; py <= maxY; py++) {
			float cy = py + 0.5F;
			int row = py * BUF_SIZE;
			for (int px = minX; px <= maxX; px++) {
				float cx = px + 0.5F;
				float w0 = ((p2[0] - p1[0]) * (cy - p1[1]) - (p2[1] - p1[1]) * (cx - p1[0])) * invArea;
				float w1 = ((p0[0] - p2[0]) * (cy - p2[1]) - (p0[1] - p2[1]) * (cx - p2[0])) * invArea;
				float w2 = 1.0F - w0 - w1;
				if (w0 < 0.0F || w1 < 0.0F || w2 < 0.0F) {
					continue;
				}
				int idx = row + px;
				float z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
				if (z >= DEPTH_BUF[idx]) {
					continue;
				}

				float invW = w0 * p0[3] + w1 * p1[3] + w2 * p2[3];
				if (invW <= 1e-6F) {
					continue;
				}
				int tu = clampInt((int) Math.floor((w0 * uw0 + w1 * uw1 + w2 * uw2) / invW), uMin, uMax) & 63;
				int tv = clampInt((int) Math.floor((w0 * vw0 + w1 * vw1 + w2 * vw2) / invW), vMin, vMax) & 63;

				int texel = SKIN_PIXELS[tv * 64 + tu];
				int ta = (texel >>> 24) & 0xFF;
				if (ta < 10) {
					continue;
				}

				int r = Math.min(255, Math.round(((texel >>> 16) & 0xFF) * shade) + rimLift);
				int g = Math.min(255, Math.round(((texel >>> 8) & 0xFF) * shade) + rimLift);
				int b = Math.min(255, Math.round((texel & 0xFF) * shade) + Math.round(rimLift * 1.15F));

				if (!outerLayer || ta >= 250) {
					DEPTH_BUF[idx] = z;
					COLOR_BUF[idx] = (0xFF << 24) | (r << 16) | (g << 8) | b;
				} else {
					int dst = COLOR_BUF[idx];
					float sa = ta / 255.0F;
					int dr = (dst >>> 16) & 0xFF;
					int dg = (dst >>> 8) & 0xFF;
					int db = dst & 0xFF;
					int da = (dst >>> 24) & 0xFF;
					int or = Math.min(255, Math.round(r * sa + dr * (1.0F - sa)));
					int og = Math.min(255, Math.round(g * sa + dg * (1.0F - sa)));
					int ob = Math.min(255, Math.round(b * sa + db * (1.0F - sa)));
					int oa = Math.min(255, ta + Math.round(da * (1.0F - sa)));
					COLOR_BUF[idx] = (oa << 24) | (or << 16) | (og << 8) | ob;
				}
			}
		}
	}

	private static void ensureSkinLoaded() {
		String wanted = FnHConfig.skinOverrideUser == null ? "" : FnHConfig.skinOverrideUser.trim();
		String key;
		String uuidTarget = null;
		String nameTarget = null;

		if (!wanted.isEmpty()) {
			key = "override:" + wanted.toLowerCase(Locale.ROOT);
			String clean = wanted.replace("-", "");
			if (clean.length() == 32 && clean.matches("[0-9a-fA-F]{32}")) {
				uuidTarget = clean.toLowerCase(Locale.ROOT);
			} else {
				nameTarget = wanted;
			}
		} else {
			Minecraft mc = Minecraft.getInstance();
			UUID uuid = mc.getUser().getProfileId();
			String name = mc.getUser().getName();
			if (uuid != null && uuid.version() == 4) {
				key = "msa_uuid:" + uuid;
				uuidTarget = uuid.toString().replace("-", "").toLowerCase(Locale.ROOT);
				nameTarget = name;
			} else if (name != null && !name.isBlank()) {
				key = "name:" + name.toLowerCase(Locale.ROOT);
				nameTarget = name;
			} else {
				key = "default:bufferclick";
				uuidTarget = BUFFERCLICK_UUID;
			}
		}

		if (key.equals(loadedSkinKey) || fetchInFlight) {
			return;
		}
		loadedSkinKey = key;
		fetchInFlight = true;

		final String finalUuid = uuidTarget;
		final String finalName = nameTarget;
		CompletableFuture.runAsync(() -> {
			try {
				HttpClient client = HttpClient.newBuilder()
					.connectTimeout(Duration.ofSeconds(5))
					.followRedirects(HttpClient.Redirect.NORMAL)
					.build();

				String resolvedUuid = finalUuid;
				if (resolvedUuid == null && finalName != null && !finalName.isBlank()) {
					if (finalName.equalsIgnoreCase("bufferclick")) {
						resolvedUuid = BUFFERCLICK_UUID;
					} else {
						resolvedUuid = resolveMojangUuidByName(client, finalName);
					}
				}

				// 1. Live official Mojang Session Server -> content-addressed `textures.minecraft.net` (ZERO stale cache!)
				if (resolvedUuid != null && loadLiveMojangSessionSkin(client, resolvedUuid)) {
					return;
				}

				// 2. Offline / firewall fallback only if Mojang sessionserver could not be reached
				String fallbackTarget = resolvedUuid != null ? resolvedUuid : (finalName != null ? finalName : BUFFERCLICK_UUID);
				for (String fallbackUrl : new String[] {
					"https://crafthead.net/skin/" + fallbackTarget,
					"https://minotar.net/skin/" + fallbackTarget
				}) {
					if (downloadAndIngestPng(client, fallbackUrl, null)) {
						return;
					}
				}
			} finally {
				fetchInFlight = false;
			}
		});
	}

	private static String resolveMojangUuidByName(HttpClient client, String username) {
		try {
			HttpRequest req = HttpRequest.newBuilder(
				URI.create("https://api.mojang.com/users/profiles/minecraft/" + username.trim())
			)
				.timeout(Duration.ofSeconds(5))
				.header("Accept", "application/json")
				.header("Cache-Control", "no-cache")
				.GET()
				.build();
			HttpResponse<String> resp = client.send(req, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
			if (resp.statusCode() == 200 && resp.body() != null) {
				Matcher m = UUID_ID_PATTERN.matcher(resp.body());
				if (m.find()) {
					return m.group(1).toLowerCase(Locale.ROOT);
				}
			}
		} catch (Exception ignored) {
		}
		return null;
	}

	private static boolean loadLiveMojangSessionSkin(HttpClient client, String undashedUuid) {
		try {
			HttpRequest req = HttpRequest.newBuilder(
				URI.create("https://sessionserver.mojang.com/session/minecraft/profile/" + undashedUuid)
			)
				.timeout(Duration.ofSeconds(5))
				.header("Accept", "application/json")
				.header("Cache-Control", "no-cache")
				.header("Pragma", "no-cache")
				.GET()
				.build();
			HttpResponse<String> resp = client.send(req, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
			if (resp.statusCode() != 200 || resp.body() == null) {
				return false;
			}
			Matcher valMatcher = TEXTURES_VALUE_PATTERN.matcher(resp.body());
			if (!valMatcher.find()) {
				return false;
			}
			String decodedJson = new String(Base64.getDecoder().decode(valMatcher.group(1)), StandardCharsets.UTF_8);
			Matcher skinBlockMatcher = SKIN_BLOCK_PATTERN.matcher(decodedJson);
			String searchScope = skinBlockMatcher.find() ? skinBlockMatcher.group(1) : decodedJson;

			Matcher urlMatcher = SKIN_URL_PATTERN.matcher(searchScope);
			if (!urlMatcher.find()) {
				return false;
			}
			String skinUrl = urlMatcher.group(1).replace("\\/", "/");
			if (skinUrl.startsWith("http://textures.minecraft.net/")) {
				skinUrl = "https://" + skinUrl.substring("http://".length());
			}
			Boolean explicitSlim = SLIM_MODEL_PATTERN.matcher(searchScope).find() ? Boolean.TRUE : Boolean.FALSE;
			return downloadAndIngestPng(client, skinUrl, explicitSlim);
		} catch (Exception ignored) {
			return false;
		}
	}

	private static boolean downloadAndIngestPng(HttpClient client, String url, Boolean explicitSlim) {
		try {
			HttpRequest req = HttpRequest.newBuilder(URI.create(url))
				.timeout(Duration.ofSeconds(6))
				.header("User-Agent", "FnH-UI/1.9")
				.header("Cache-Control", "no-cache")
				.GET()
				.build();
			HttpResponse<byte[]> resp = client.send(req, HttpResponse.BodyHandlers.ofByteArray());
			if (resp.statusCode() == 200 && resp.body() != null && resp.body().length > 64) {
				try (NativeImage img = NativeImage.read(new ByteArrayInputStream(resp.body()))) {
					if (img.getWidth() >= 64 && (img.getHeight() == 64 || img.getHeight() == 32)) {
						ingestNativeSkin(img, explicitSlim);
						return true;
					}
				}
			}
		} catch (Exception ignored) {
		}
		return false;
	}

	private static void ingestNativeSkin(NativeImage img, Boolean explicitSlim) {
		int[] next = new int[64 * 64];
		int h = Math.min(64, img.getHeight());
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < 64; x++) {
				next[y * 64 + x] = img.getPixel(x, y);
			}
		}
		if (img.getHeight() == 32) {
			copyBlock(next, 0, 16, 16, 48, 16, 16);
			copyBlock(next, 40, 16, 32, 48, 16, 16);
		}
		slimArms = explicitSlim != null ? explicitSlim : (((next[20 * 64 + 54] >>> 24) & 0xFF) == 0);
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
		fillSkinRect(0, 0, 32, 16, 0xFF28A45A);
		fillSkinRect(9, 11, 2, 2, 0xFF101318);
		fillSkinRect(13, 11, 2, 2, 0xFF101318);
		fillSkinRect(10, 14, 4, 1, 0xFF101318);
		fillSkinRect(16, 16, 24, 16, 0xFF101218);
		fillSkinRect(40, 16, 16, 16, 0xFF12151C);
		fillSkinRect(32, 48, 16, 16, 0xFF12151C);
		fillSkinRect(0, 16, 16, 16, 0xFF0E1016);
		fillSkinRect(16, 48, 16, 16, 0xFF0E1016);
	}

	private static void fillSkinRect(int x0, int y0, int w, int h, int argb) {
		for (int y = y0; y < y0 + h; y++) {
			for (int x = x0; x < x0 + w; x++) {
				SKIN_PIXELS[y * 64 + x] = argb;
			}
		}
	}

	private static int clampInt(int v, int lo, int hi) {
		return Math.max(lo, Math.min(hi, v));
	}

	private static float clamp(float v, float lo, float hi) {
		return Math.max(lo, Math.min(hi, v));
	}

	private static float smoothstep(float e0, float e1, float x) {
		float t = clamp((x - e0) / (e1 - e0), 0.0F, 1.0F);
		return t * t * (3.0F - 2.0F * t);
	}

	private static float walkEase(float t) {
		if (t < 0.72F) {
			return (t / 0.72F) * 0.82F;
		}
		float u = (t - 0.72F) / 0.28F;
		return 0.82F + (1.0F - (float) Math.pow(1.0F - u, 3.0)) * 0.18F;
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

	private static float[] matPersp(float fovY, float aspect, float near, float far) {
		float f = 1.0F / (float) Math.tan(fovY * 0.5F);
		float nf = 1.0F / (near - far);
		return new float[] {
			f / aspect, 0, 0, 0,
			0, f, 0, 0,
			0, 0, (far + near) * nf, -1,
			0, 0, (2 * far * near) * nf, 0
		};
	}

	private static float[] matMul(float[] a, float[] b) {
		float[] o = new float[16];
		for (int col = 0; col < 4; col++) {
			int c4 = col * 4;
			float b0 = b[c4], b1 = b[c4 + 1], b2 = b[c4 + 2], b3 = b[c4 + 3];
			o[c4] = a[0] * b0 + a[4] * b1 + a[8] * b2 + a[12] * b3;
			o[c4 + 1] = a[1] * b0 + a[5] * b1 + a[9] * b2 + a[13] * b3;
			o[c4 + 2] = a[2] * b0 + a[6] * b1 + a[10] * b2 + a[14] * b3;
			o[c4 + 3] = a[3] * b0 + a[7] * b1 + a[11] * b2 + a[15] * b3;
		}
		return o;
	}
}
