package dev.horizonlab.fnh;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;

public final class FnHGlassStyle {
	private static int mouseX = -9999;
	private static int mouseY = -9999;
	private static Class<?> lastScreenClass = null;
	private static long screenEnterMs = 0L;

	private FnHGlassStyle() {
	}

	public static void beginScreenFrame(Screen screen, int mx, int my) {
		mouseX = mx;
		mouseY = my;
		if (screen != null && screen.getClass() != lastScreenClass) {
			lastScreenClass = screen.getClass();
			screenEnterMs = Util.getMillis();
		}
	}

	public static float currentSlideOffsetY() {
		if (FnHConfig.motionBlurPercent <= 0) {
			return 0.0F;
		}
		long elapsed = Util.getMillis() - screenEnterMs;
		if (elapsed < 0L || elapsed >= 210L) {
			return 0.0F;
		}
		float p = elapsed / 210.0F;
		float ease = 1.0F - (float) Math.pow(1.0F - p, 3.0);
		float mult = Math.min(1.5F, FnHConfig.motionBlurPercent / 100.0F);
		return (1.0F - ease) * 7.5F * mult;
	}

	public static boolean tryRenderGlassWidget(GuiGraphics g, Identifier sprite, int x, int y, int w, int h, int tintColor) {
		if (sprite == null || w <= 0 || h <= 0) {
			return false;
		}
		if (!"minecraft".equals(sprite.getNamespace())) {
			return false;
		}
		String path = sprite.getPath();

		float extAlpha = (((tintColor >>> 24) & 0xFF) == 0 ? 255 : ((tintColor >>> 24) & 0xFF)) / 255.0F;

		if (path.equals("widget/button") || path.equals("widget/button_highlighted") || path.equals("widget/button_disabled")) {
			boolean disabled = path.endsWith("_disabled");
			boolean hovered = path.endsWith("_highlighted");
			int bg = disabled
				? withAlpha(0x0A0C12, 0.42F * extAlpha)
				: hovered
					? withAlpha(0x181C2A, 0.84F * extAlpha)
					: withAlpha(0x0C0E16, 0.70F * extAlpha);
			int border = disabled
				? withAlpha(0xFFFFFF, 0.08F * extAlpha)
				: hovered
					? withAlpha(0xFFFFFF, 0.34F * extAlpha)
					: withAlpha(0xFFFFFF, 0.16F * extAlpha);

			g.fill(x, y, x + w, y + h, bg);
			// Subtle inner top glass sheen
			if (!disabled && h >= 6) {
				g.fill(x + 1, y + 1, x + w - 1, y + 2, withAlpha(0xFFFFFF, (hovered ? 0.09F : 0.04F) * extAlpha));
			}
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, !disabled, extAlpha);
			return true;
		}

		if (path.equals("widget/slider") || path.equals("widget/slider_highlighted")) {
			boolean hovered = path.endsWith("_highlighted");
			int bg = withAlpha(0x070911, (hovered ? 0.80F : 0.74F) * extAlpha);
			int border = withAlpha(0xFFFFFF, (hovered ? 0.30F : 0.16F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, true, extAlpha);
			return true;
		}

		if (path.equals("widget/slider_handle") || path.equals("widget/slider_handle_highlighted")) {
			boolean hovered = path.endsWith("_highlighted");
			int fill = withAlpha(0xE6ECF8, (hovered ? 0.56F : 0.40F) * extAlpha);
			int border = withAlpha(0xFFFFFF, (hovered ? 0.78F : 0.56F) * extAlpha);
			g.fill(x, y + 1, x + w, y + h - 1, fill);
			drawRectBorder(g, x, y + 1, w, h - 2, border);
			return true;
		}

		if (path.equals("widget/text_field") || path.equals("widget/text_field_highlighted")) {
			boolean focused = path.endsWith("_highlighted");
			int bg = withAlpha(0x05070E, 0.84F * extAlpha);
			int border = withAlpha(0xFFFFFF, (focused ? 0.62F : 0.24F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, true, extAlpha);
			return true;
		}

		if (path.startsWith("widget/tab")) {
			boolean selected = path.contains("selected");
			boolean hovered = path.contains("highlighted");
			int bg = selected
				? withAlpha(0x141826, 0.82F * extAlpha)
				: hovered
					? withAlpha(0x121622, 0.76F * extAlpha)
					: withAlpha(0x090B13, 0.68F * extAlpha);
			int border = withAlpha(0xFFFFFF, (selected ? 0.34F : 0.16F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			g.fill(x, y, x + w, y + 1, border);
			g.fill(x, y, x + 1, y + h, border);
			g.fill(x + w - 1, y, x + w, y + h, border);
			if (selected && w > 16) {
				g.fill(x + 8, y + h - 2, x + w - 8, y + h, withAlpha(0xFFFFFF, 0.90F * extAlpha));
			}
			return true;
		}

		return false;
	}

	private static void drawGlassBorderWithCursorRim(
		GuiGraphics g, int x, int y, int w, int h,
		int baseBorder, boolean allowRim, float extAlpha
	) {
		drawRectBorder(g, x, y, w, h, baseBorder);
		if (!allowRim) {
			return;
		}
		// Tight, subtle 38-GUI-unit cursor rim highlight (never a large glow ball!)
		int pad = 38;
		if (mouseX < x - pad || mouseX > x + w + pad || mouseY < y - pad || mouseY > y + h + pad) {
			return;
		}
		float radiusSq = 38.0F * 38.0F;
		int step = 4;

		for (int sx = x; sx < x + w; sx += step) {
			int ex = Math.min(x + w, sx + step);
			float cx = (sx + ex) * 0.5F;

			float dTop = (cx - mouseX) * (cx - mouseX) + (y - mouseY) * (y - mouseY);
			if (dTop < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dTop / radiusSq);
				g.fill(sx, y, ex, y + 1, withAlpha(0xFFFFFF, f * f * 0.40F * extAlpha));
			}

			float by = y + h - 1;
			float dBot = (cx - mouseX) * (cx - mouseX) + (by - mouseY) * (by - mouseY);
			if (dBot < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dBot / radiusSq);
				g.fill(sx, y + h - 1, ex, y + h, withAlpha(0xFFFFFF, f * f * 0.40F * extAlpha));
			}
		}

		for (int sy = y + 1; sy < y + h - 1; sy += step) {
			int ey = Math.min(y + h - 1, sy + step);
			float cy = (sy + ey) * 0.5F;

			float dLeft = (x - mouseX) * (x - mouseX) + (cy - mouseY) * (cy - mouseY);
			if (dLeft < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dLeft / radiusSq);
				g.fill(x, sy, x + 1, ey, withAlpha(0xFFFFFF, f * f * 0.40F * extAlpha));
			}

			float rx = x + w - 1;
			float dRight = (rx - mouseX) * (rx - mouseX) + (cy - mouseY) * (cy - mouseY);
			if (dRight < radiusSq) {
				float f = 1.0F - (float) Math.sqrt(dRight / radiusSq);
				g.fill(x + w - 1, sy, x + w, ey, withAlpha(0xFFFFFF, f * f * 0.40F * extAlpha));
			}
		}
	}

	private static void drawRectBorder(GuiGraphics g, int x, int y, int w, int h, int color) {
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
