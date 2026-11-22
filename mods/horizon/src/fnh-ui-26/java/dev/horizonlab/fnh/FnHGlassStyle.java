package dev.horizonlab.fnh;

import java.util.ArrayList;
import java.util.List;

import dev.horizonlab.fnh.mixin.ScreenAccessor;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Renderable;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;

public final class FnHGlassStyle {
	private static int mouseX = -9999;
	private static int mouseY = -9999;

	private static final List<Class<?>> menuHistory = new ArrayList<>();
	private static long openAnimStartMs = 0L;
	private static Screen closingScreen = null;
	private static long closeAnimStartMs = 0L;
	private static boolean renderingClosingOverlay = false;
	private static float activeOverlayAlpha = 1.0F;

	private static float smoothHotbarX = Float.NaN;
	private static int lastHotbarY = -9999;
	private static long lastHotbarNs = 0L;

	private FnHGlassStyle() {
	}

	public static boolean isInGame() {
		Minecraft mc = Minecraft.getInstance();
		return mc != null && mc.level != null;
	}

	public static void onSetScreen(Screen oldScreen, Screen newScreen) {
		if (isInGame() || newScreen == null || newScreen instanceof AbstractContainerScreen<?>) {
			menuHistory.clear();
			openAnimStartMs = 0L;
			closingScreen = null;
			return;
		}
		if (oldScreen == null || oldScreen == newScreen || oldScreen.getClass() == newScreen.getClass()) {
			return;
		}

		if (newScreen instanceof TitleScreen) {
			menuHistory.clear();
			openAnimStartMs = 0L;
			if (!(oldScreen instanceof AbstractContainerScreen<?>)) {
				closingScreen = oldScreen;
				closeAnimStartMs = Util.getMillis();
			}
			return;
		}

		int existingIdx = menuHistory.indexOf(newScreen.getClass());
		if (existingIdx >= 0) {
			while (menuHistory.size() > existingIdx + 1) {
				menuHistory.remove(menuHistory.size() - 1);
			}
			openAnimStartMs = 0L;
			closingScreen = oldScreen;
			closeAnimStartMs = Util.getMillis();
		} else {
			menuHistory.add(newScreen.getClass());
			openAnimStartMs = Util.getMillis();
			closingScreen = null;
		}
	}

	public static void beginScreenFrame(Screen screen, int mx, int my) {
		if (!renderingClosingOverlay) {
			mouseX = mx;
			mouseY = my;
		}
		if (isInGame() || screen instanceof AbstractContainerScreen<?>) {
			openAnimStartMs = 0L;
			closingScreen = null;
		}
	}

	public static float currentOpenSlideUpY(Screen screen) {
		if (renderingClosingOverlay || isInGame() || screen == null
			|| screen instanceof TitleScreen || screen instanceof AbstractContainerScreen<?>) {
			return 0.0F;
		}
		if (FnHConfig.motionBlurPercent <= 0 || openAnimStartMs == 0L) {
			return 0.0F;
		}
		long elapsed = Util.getMillis() - openAnimStartMs;
		if (elapsed < 0L || elapsed >= 255L) {
			return 0.0F;
		}
		float p = elapsed / 255.0F;
		float easeOut = 1.0F - (float) Math.pow(1.0F - p, 3.0);
		float mult = Math.max(0.45F, Math.min(1.5F, FnHConfig.motionBlurPercent / 100.0F));
		return (1.0F - easeOut) * 54.0F * mult;
	}

	public static void renderMenuMotionBlur(GuiGraphicsExtractor g, Screen screen, float speed01, float trailSign) {
		if (FnHConfig.motionBlurPercent <= 0 || speed01 <= 0.03F || isInGame()) {
			return;
		}
		float blurScale = Math.min(1.5F, FnHConfig.motionBlurPercent / 100.0F);
		float maxStreak = 22.0F * speed01 * blurScale;
		if (maxStreak < 1.2F) {
			return;
		}

		List<GuiEventListener> kids = ((ScreenAccessor) screen).fnh_getChildren();
		for (GuiEventListener listener : kids) {
			if (!(listener instanceof AbstractWidget w) || !w.visible) {
				continue;
			}
			int wx = w.getX();
			int wy = w.getY();
			int ww = w.getWidth();
			int wh = w.getHeight();
			for (int tap = 1; tap <= 4; tap++) {
				float frac = tap / 4.0F;
				int dy = Math.round(trailSign * maxStreak * frac);
				float a = (1.0F - frac * 0.72F) * 0.16F * speed01 * activeOverlayAlpha;
				g.fill(wx, wy + dy, wx + ww, wy + wh + dy, withAlpha(0x161D2E, a));
				g.fill(wx, wy + dy, wx + ww, wy + 1 + dy, withAlpha(0xDCE6FF, a * 0.65F));
			}
		}
	}

	public static void renderClosingMenuOverlay(GuiGraphicsExtractor g, float deltaTicks) {
		if (closingScreen == null || renderingClosingOverlay || isInGame()) {
			closingScreen = null;
			return;
		}
		if (FnHConfig.motionBlurPercent <= 0) {
			closingScreen = null;
			return;
		}
		long elapsed = Util.getMillis() - closeAnimStartMs;
		if (elapsed < 0L || elapsed >= 220L) {
			closingScreen = null;
			return;
		}

		float p = elapsed / 220.0F;
		float easeIn = p * p * (2.1F - 1.1F * p);
		float mult = Math.max(0.45F, Math.min(1.5F, FnHConfig.motionBlurPercent / 100.0F));
		float slideDownY = easeIn * 72.0F * mult;
		float fade = Math.max(0.0F, 1.0F - p * 1.08F);
		if (fade <= 0.02F) {
			closingScreen = null;
			return;
		}

		Screen outgoing = closingScreen;
		renderingClosingOverlay = true;
		activeOverlayAlpha = fade;
		try {
			g.pose().pushMatrix();
			g.pose().translate(0.0F, slideDownY);
			float instSpeed = Math.min(1.0F, 0.35F + p * 0.95F);
			renderMenuMotionBlur(g, outgoing, instSpeed, -1.0F);
			for (Renderable r : ((ScreenAccessor) outgoing).fnh_getRenderables()) {
				r.extractRenderState(g, -9999, -9999, deltaTicks);
			}
			g.pose().popMatrix();
		} catch (Throwable ignored) {
			closingScreen = null;
		} finally {
			activeOverlayAlpha = 1.0F;
			renderingClosingOverlay = false;
		}
	}

	public static int upgradeContainerLabelColor(int color) {
		if ((color & 0x00FFFFFF) == 0x404040) {
			Minecraft mc = Minecraft.getInstance();
			if (mc != null && mc.gui != null && mc.gui.screen() instanceof AbstractContainerScreen<?>) {
				int a = (color >>> 24) & 0xFF;
				if (a == 0) {
					a = 0xFF;
				}
				return (a << 24) | 0xEDF2FC;
			}
		}
		return color;
	}

	public static boolean tryRenderGlassWidget(
		GuiGraphicsExtractor g,
		Identifier sprite,
		int x,
		int y,
		int w,
		int h,
		int tintColor,
		boolean hasExplicitAlpha
	) {
		if (sprite == null || w <= 0 || h <= 0 || !"minecraft".equals(sprite.getNamespace())) {
			return false;
		}
		String path = sprite.getPath();

		float rawAlpha = hasExplicitAlpha ? (((tintColor >>> 24) & 0xFF) / 255.0F) : 1.0F;
		float extAlpha = rawAlpha * activeOverlayAlpha;
		if (extAlpha <= 0.004F) {
			return true;
		}

		if (path.equals("hud/hotbar_selection")) {
			long nowNs = System.nanoTime();
			float dt = lastHotbarNs == 0L ? 0.016F : Math.min(0.1F, Math.max(0.0005F, (nowNs - lastHotbarNs) * 1.0e-9F));
			lastHotbarNs = nowNs;

			if (Float.isNaN(smoothHotbarX) || Math.abs(y - lastHotbarY) > 4 || Math.abs(x - smoothHotbarX) > 220.0F) {
				smoothHotbarX = x;
				lastHotbarY = y;
			} else {
				lastHotbarY = y;
				float k = 1.0F - (float) Math.exp(-dt * 28.0F);
				smoothHotbarX += (x - smoothHotbarX) * k;
				if (Math.abs(x - smoothHotbarX) < 0.05F) {
					smoothHotbarX = x;
				}
			}

			g.pose().pushMatrix();
			g.pose().translate(smoothHotbarX - x, 0.0F);
			drawRectBorder(g, x, y, w, h, withAlpha(0x9CB8FF, 0.42F * extAlpha));
			g.fill(x + 2, y + 2, x + w - 2, y + h - 2, withAlpha(0xDCE8FF, 0.12F * extAlpha));
			drawRectBorder(g, x + 1, y + 1, w - 2, h - 2, withAlpha(0xF6F9FF, 0.96F * extAlpha));
			drawRectBorder(g, x + 2, y + 2, w - 4, h - 4, withAlpha(0xDCE8FF, 0.72F * extAlpha));
			g.pose().popMatrix();
			return true;
		}

		if (path.equals("hud/hotbar")) {
			g.fill(x, y, x + w, y + h, withAlpha(0x0B0E17, 0.82F * extAlpha));
			g.fill(x + 1, y + 1, x + w - 1, y + 2, withAlpha(0xDCE8FF, 0.14F * extAlpha));
			drawRectBorder(g, x, y, w, h, withAlpha(0x6C7B9E, 0.72F * extAlpha));
			for (int slot = 0; slot < 9; slot++) {
				int sx = x + 3 + slot * 20;
				int sy = y + 3;
				g.fill(sx, sy, sx + 16, sy + 16, withAlpha(0x05070C, 0.62F * extAlpha));
				drawRectBorder(g, sx, sy, 16, 16, withAlpha(0xFFFFFF, 0.07F * extAlpha));
			}
			return true;
		}

		if (path.equals("hud/hotbar_offhand_left") || path.equals("hud/hotbar_offhand_right")) {
			int ox = path.endsWith("_left") ? x : (x + w - 22);
			g.fill(ox, y + 1, ox + 22, y + h - 1, withAlpha(0x0B0E17, 0.82F * extAlpha));
			drawRectBorder(g, ox, y + 1, 22, h - 2, withAlpha(0x6C7B9E, 0.72F * extAlpha));
			g.fill(ox + 3, y + 4, ox + 19, y + 20, withAlpha(0x05070C, 0.62F * extAlpha));
			return true;
		}

		if (path.equals("widget/button") || path.equals("widget/button_highlighted") || path.equals("widget/button_disabled")) {
			boolean disabled = path.endsWith("_disabled");
			boolean hovered = path.endsWith("_highlighted") && !renderingClosingOverlay;
			int bg = disabled
				? withAlpha(0x0A0C12, 0.42F * extAlpha)
				: hovered
					? withAlpha(0x1A2032, 0.86F * extAlpha)
					: withAlpha(0x0C0F18, 0.74F * extAlpha);
			int border = disabled
				? withAlpha(0xFFFFFF, 0.08F * extAlpha)
				: hovered
					? withAlpha(0xDCE8FF, 0.48F * extAlpha)
					: withAlpha(0xDCE6FF, 0.19F * extAlpha);

			g.fill(x, y, x + w, y + h, bg);
			if (!disabled && h >= 6) {
				g.fill(x + 1, y + 1, x + w - 1, y + 2, withAlpha(0xFFFFFF, (hovered ? 0.12F : 0.05F) * extAlpha));
			}
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, !disabled && !renderingClosingOverlay, extAlpha);
			return true;
		}

		if (path.equals("widget/slider") || path.equals("widget/slider_highlighted")) {
			boolean hovered = path.endsWith("_highlighted") && !renderingClosingOverlay;
			int bg = withAlpha(0x070911, (hovered ? 0.82F : 0.75F) * extAlpha);
			int border = withAlpha(0xDCE6FF, (hovered ? 0.34F : 0.18F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, !renderingClosingOverlay, extAlpha);
			return true;
		}

		if (path.equals("widget/slider_handle") || path.equals("widget/slider_handle_highlighted")) {
			boolean hovered = path.endsWith("_highlighted") && !renderingClosingOverlay;
			int fill = withAlpha(0xE6ECF8, (hovered ? 0.62F : 0.44F) * extAlpha);
			int border = withAlpha(0xFFFFFF, (hovered ? 0.84F : 0.60F) * extAlpha);
			g.fill(x, y + 1, x + w, y + h - 1, fill);
			drawRectBorder(g, x, y + 1, w, h - 2, border);
			return true;
		}

		if (path.equals("widget/text_field") || path.equals("widget/text_field_highlighted")) {
			boolean focused = path.endsWith("_highlighted");
			int bg = withAlpha(0x05070E, 0.86F * extAlpha);
			int border = withAlpha(0xDCE8FF, (focused ? 0.66F : 0.24F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, !renderingClosingOverlay, extAlpha);
			return true;
		}

		if (path.startsWith("widget/tab")) {
			boolean selected = path.contains("selected");
			boolean hovered = path.contains("highlighted");
			int bg = selected
				? withAlpha(0x161C2E, 0.86F * extAlpha)
				: hovered
					? withAlpha(0x121726, 0.78F * extAlpha)
					: withAlpha(0x090C14, 0.70F * extAlpha);
			int border = withAlpha(0xDCE6FF, (selected ? 0.38F : 0.18F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			g.fill(x, y, x + w, y + 1, border);
			g.fill(x, y, x + 1, y + h, border);
			g.fill(x + w - 1, y, x + w, y + h, border);
			if (selected && w > 16) {
				g.fill(x + 8, y + h - 2, x + w - 8, y + h, withAlpha(0xFFFFFF, 0.92F * extAlpha));
			}
			return true;
		}

		return false;
	}

	private static void drawGlassBorderWithCursorRim(
		GuiGraphicsExtractor g, int x, int y, int w, int h,
		int baseBorder, boolean allowRim, float extAlpha
	) {
		drawRectBorder(g, x, y, w, h, baseBorder);
		if (!allowRim) {
			return;
		}
		int pad = 36;
		if (mouseX < x - pad || mouseX > x + w + pad || mouseY < y - pad || mouseY > y + h + pad) {
			return;
		}
		float radiusSq = 36.0F * 36.0F;
		int step = 4;

		for (int sx = x; sx < x + w; sx += step) {
			int ex = Math.min(x + w, sx + step);
			float cx = (sx + ex) * 0.5F;

			float dTop = (cx - mouseX) * (cx - mouseX) + (y - mouseY) * (y - mouseY);
			if (dTop < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dTop / radiusSq);
				g.fill(sx, y, ex, y + 1, withAlpha(0xFFFFFF, f * f * 0.38F * extAlpha));
			}

			float by = y + h - 1;
			float dBot = (cx - mouseX) * (cx - mouseX) + (by - mouseY) * (by - mouseY);
			if (dBot < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dBot / radiusSq);
				g.fill(sx, y + h - 1, ex, y + h, withAlpha(0xFFFFFF, f * f * 0.38F * extAlpha));
			}
		}

		for (int sy = y + 1; sy < y + h - 1; sy += step) {
			int ey = Math.min(y + h - 1, sy + step);
			float cy = (sy + ey) * 0.5F;

			float dLeft = (x - mouseX) * (x - mouseX) + (cy - mouseY) * (cy - mouseY);
			if (dLeft < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dLeft / radiusSq);
				g.fill(x, sy, x + 1, ey, withAlpha(0xFFFFFF, f * f * 0.38F * extAlpha));
			}

			float rx = x + w - 1;
			float dRight = (rx - mouseX) * (rx - mouseX) + (cy - mouseY) * (cy - mouseY);
			if (dRight < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dRight / radiusSq);
				g.fill(x + w - 1, sy, x + w, ey, withAlpha(0xFFFFFF, f * f * 0.38F * extAlpha));
			}
		}
	}

	private static void drawRectBorder(GuiGraphicsExtractor g, int x, int y, int w, int h, int color) {
		g.fill(x, y, x + w, y + 1, color);
		g.fill(x, y + h - 1, x + w, y + h, color);
		if (h > 2) {
			g.fill(x, y + 1, x + 1, y + h - 1, color);
			g.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
		}
	}

	private static int withAlpha(int rgb, float alpha01) {
		int a = Math.max(0, Math.min(255, Math.round(alpha01 * 255.0F)));
		return (a << 24) | (rgb & 0x00FFFFFF);
	}
}
