package dev.horizonlab.fnh;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;

public final class FnHGlassStyle {
	private static final Identifier BLUR_LOW = Identifier.withDefaultNamespace("fnh_motion_blur_low");
	private static final Identifier BLUR_MED = Identifier.withDefaultNamespace("fnh_motion_blur_med");
	private static final Identifier BLUR_HIGH = Identifier.withDefaultNamespace("fnh_motion_blur_high");
	private static final Set<Identifier> MAIN_TARGETS = Set.of(Identifier.withDefaultNamespace("main"));

	private static boolean postBridgeResolved = false;
	private static Method getPostChainMethod = null;
	private static Method processChainMethod = null;
	private static Method mainTargetGetter = null;
	private static Field mainTargetField = null;

	private static int mouseX = -9999;
	private static int mouseY = -9999;

	private static final List<Class<?>> menuHistory = new ArrayList<>();
	private static long animStartMs = 0L;
	private static float animInitialOffsetY = 0.0F;
	private static long animDurationMs = 165L;

	private static float smoothHotbarX = Float.NaN;
	private static int lastHotbarY = -9999;
	private static long lastHotbarNs = 0L;

	private FnHGlassStyle() {
	}

	public static boolean isInGame() {
		Minecraft mc = Minecraft.getInstance();
		return mc != null && mc.level != null;
	}

	private static void ensurePostBridge(Minecraft mc) {
		if (postBridgeResolved) {
			return;
		}
		postBridgeResolved = true;
		try {
			Object shaderMgr = mc.getShaderManager();
			for (Method m : shaderMgr.getClass().getMethods()) {
				if ("getPostChain".equals(m.getName()) && m.getParameterCount() == 2) {
					getPostChainMethod = m;
					Class<?> postChainCls = m.getReturnType();
					for (Method pm : postChainCls.getMethods()) {
						if ("process".equals(pm.getName()) && pm.getParameterCount() == 2) {
							processChainMethod = pm;
							break;
						}
					}
					break;
				}
			}
			for (String mName : new String[] { "mainRenderTarget", "getMainRenderTarget" }) {
				try {
					mainTargetGetter = Minecraft.class.getMethod(mName);
					break;
				} catch (NoSuchMethodException ignored) {
				}
			}
			if (mainTargetGetter == null && processChainMethod != null) {
				Class<?> targetCls = processChainMethod.getParameterTypes()[0];
				for (Field f : Minecraft.class.getDeclaredFields()) {
					if (targetCls.isAssignableFrom(f.getType())) {
						f.setAccessible(true);
						mainTargetField = f;
						break;
					}
				}
			}
		} catch (Throwable ignored) {
		}
	}

	public static void applyInGameMotionBlur(Object resourceAllocator) {
		if (FnHConfig.motionBlurPercent <= 0 || resourceAllocator == null) {
			return;
		}
		Minecraft mc = Minecraft.getInstance();
		if (mc == null || mc.level == null) {
			return;
		}
		ensurePostBridge(mc);
		if (getPostChainMethod == null || processChainMethod == null) {
			return;
		}
		Identifier wanted = FnHConfig.motionBlurPercent <= 55
			? BLUR_LOW
			: (FnHConfig.motionBlurPercent <= 105 ? BLUR_MED : BLUR_HIGH);
		try {
			Object target = mainTargetGetter != null
				? mainTargetGetter.invoke(mc)
				: (mainTargetField != null ? mainTargetField.get(mc) : null);
			if (target == null) {
				return;
			}
			Object chain = getPostChainMethod.invoke(mc.getShaderManager(), wanted, MAIN_TARGETS);
			if (chain != null) {
				processChainMethod.invoke(chain, target, resourceAllocator);
			}
		} catch (Throwable ignored) {
		}
	}

	public static void onSetScreen(Screen oldScreen, Screen newScreen) {
		if (isInGame() || newScreen == null || newScreen instanceof AbstractContainerScreen<?>) {
			menuHistory.clear();
			animStartMs = 0L;
			animInitialOffsetY = 0.0F;
			return;
		}
		if (oldScreen == null || oldScreen == newScreen || oldScreen.getClass() == newScreen.getClass()) {
			return;
		}

		if (newScreen instanceof TitleScreen) {
			menuHistory.clear();
			animStartMs = 0L;
			animInitialOffsetY = 0.0F;
			return;
		}

		int existingIdx = menuHistory.indexOf(newScreen.getClass());
		if (existingIdx >= 0) {
			while (menuHistory.size() > existingIdx + 1) {
				menuHistory.remove(menuHistory.size() - 1);
			}
			animStartMs = Util.getMillis();
			animInitialOffsetY = -10.0F;
			animDurationMs = 150L;
		} else {
			menuHistory.add(newScreen.getClass());
			animStartMs = Util.getMillis();
			animInitialOffsetY = 14.0F;
			animDurationMs = 165L;
		}
	}

	public static void beginScreenFrame(Screen screen, int mx, int my) {
		mouseX = mx;
		mouseY = my;
		if (isInGame() || screen instanceof AbstractContainerScreen<?>) {
			animStartMs = 0L;
			animInitialOffsetY = 0.0F;
		}
	}

	public static float currentOpenSlideUpY(Screen screen) {
		if (isInGame() || screen == null
			|| screen instanceof TitleScreen || screen instanceof AbstractContainerScreen<?>) {
			return 0.0F;
		}
		if (animStartMs == 0L || animInitialOffsetY == 0.0F || animDurationMs <= 0L) {
			return 0.0F;
		}
		long elapsed = Util.getMillis() - animStartMs;
		if (elapsed < 0L || elapsed >= animDurationMs) {
			return 0.0F;
		}
		float p = (float) elapsed / (float) animDurationMs;
		float easeOut = 1.0F - (float) Math.pow(1.0F - p, 3.0);
		return (1.0F - easeOut) * animInitialOffsetY;
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

		float extAlpha = hasExplicitAlpha ? (((tintColor >>> 24) & 0xFF) / 255.0F) : 1.0F;
		if (extAlpha <= 0.004F) {
			return true;
		}

		// Clean, minimal sliding white slot frame
		if (path.equals("hud/hotbar_selection")) {
			long nowNs = System.nanoTime();
			float dt = lastHotbarNs == 0L ? 0.016F : Math.min(0.1F, Math.max(0.0005F, (nowNs - lastHotbarNs) * 1.0e-9F));
			lastHotbarNs = nowNs;

			if (Float.isNaN(smoothHotbarX) || Math.abs(y - lastHotbarY) > 4 || Math.abs(x - smoothHotbarX) > 220.0F) {
				smoothHotbarX = x;
				lastHotbarY = y;
			} else {
				lastHotbarY = y;
				float k = 1.0F - (float) Math.exp(-dt * 30.0F);
				smoothHotbarX += (x - smoothHotbarX) * k;
				if (Math.abs(x - smoothHotbarX) < 0.05F) {
					smoothHotbarX = x;
				}
			}

			g.pose().pushMatrix();
			g.pose().translate(smoothHotbarX - x, 0.0F);
			int sx = x + 2;
			int sy = y + 1;
			int sw = Math.max(18, w - 4);
			int sh = Math.max(18, h - 3);
			g.fill(sx + 1, sy + 1, sx + sw - 1, sy + sh - 1, withAlpha(0xFFFFFF, 0.07F * extAlpha));
			drawRectBorder(g, sx, sy, sw, sh, withAlpha(0xFFFFFF, 0.88F * extAlpha));
			g.pose().popMatrix();
			return true;
		}

		// Clean, minimal smoked-obsidian pill hotbar (no cluttered 9-box grid)
		if (path.equals("hud/hotbar")) {
			g.fill(x, y, x + w, y + h, withAlpha(0x0A0C12, 0.58F * extAlpha));
			g.fill(x + 1, y + 1, x + w - 1, y + 2, withAlpha(0xFFFFFF, 0.06F * extAlpha));
			drawRectBorder(g, x, y, w, h, withAlpha(0xFFFFFF, 0.15F * extAlpha));
			for (int div = 1; div < 9; div++) {
				int dx = x + 1 + div * 20;
				g.fill(dx, y + 5, dx + 1, y + h - 5, withAlpha(0xFFFFFF, 0.07F * extAlpha));
			}
			return true;
		}

		if (path.equals("hud/hotbar_offhand_left") || path.equals("hud/hotbar_offhand_right")) {
			int ox = path.endsWith("_left") ? x : (x + w - 22);
			g.fill(ox, y + 1, ox + 22, y + h - 1, withAlpha(0x0A0C12, 0.58F * extAlpha));
			g.fill(ox + 1, y + 2, ox + 21, y + 3, withAlpha(0xFFFFFF, 0.06F * extAlpha));
			drawRectBorder(g, ox, y + 1, 22, h - 2, withAlpha(0xFFFFFF, 0.15F * extAlpha));
			return true;
		}

		if (path.equals("widget/button") || path.equals("widget/button_highlighted") || path.equals("widget/button_disabled")) {
			boolean disabled = path.endsWith("_disabled");
			boolean hovered = path.endsWith("_highlighted");
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
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, !disabled, extAlpha);
			return true;
		}

		if (path.equals("widget/slider") || path.equals("widget/slider_highlighted")) {
			boolean hovered = path.endsWith("_highlighted");
			int bg = withAlpha(0x070911, (hovered ? 0.82F : 0.75F) * extAlpha);
			int border = withAlpha(0xDCE6FF, (hovered ? 0.34F : 0.18F) * extAlpha);
			g.fill(x, y, x + w, y + h, bg);
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, true, extAlpha);
			return true;
		}

		if (path.equals("widget/slider_handle") || path.equals("widget/slider_handle_highlighted")) {
			boolean hovered = path.endsWith("_highlighted");
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
			drawGlassBorderWithCursorRim(g, x, y, w, h, border, true, extAlpha);
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
