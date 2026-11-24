package dev.horizonlab.fnh.mixin;

import java.util.List;

import dev.horizonlab.fnh.FnHBackgroundRenderer;
import dev.horizonlab.fnh.FnHSettingsScreen;
import dev.horizonlab.fnh.FnHSkinStageRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.SharedConstants;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Renderable;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.util.ARGB;
import net.minecraft.util.Util;

@Mixin(TitleScreen.class)
public abstract class TitleScreenMixin extends Screen {
	@Unique
	private int fnh_logoY = 24;

	@Unique
	private int fnh_stageTopY = 82;

	@Unique
	private int fnh_stageSize = 96;

	@Unique
	private int fnh_bgBtnX = -100;

	@Unique
	private int fnh_bgBtnY = -100;

	protected TitleScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "init", at = @At("TAIL"))
	private void fnh_afterInit(CallbackInfo ci) {
		List<GuiEventListener> kids = ((ScreenAccessor) this).fnh_getChildren();

		int minMainY = Integer.MAX_VALUE;
		int maxMainBottom = 0;
		int bottomRowY = -1;
		int maxRightIconX = this.width / 2 + 104;

		for (GuiEventListener listener : kids) {
			if (listener instanceof AbstractWidget w && w.getY() < this.height - 20) {
				if (w.getWidth() >= 80) {
					minMainY = Math.min(minMainY, w.getY());
					maxMainBottom = Math.max(maxMainBottom, w.getY() + w.getHeight());
					bottomRowY = Math.max(bottomRowY, w.getY());
				}
			}
		}

		int logoBlockH = 49;
		int stackHeight = (minMainY != Integer.MAX_VALUE) ? Math.max(72, maxMainBottom - minMainY) : 96;

		int availForSkin = this.height - logoBlockH - stackHeight - 40;
		this.fnh_stageSize = Math.max(44, Math.min(100, availForSkin));
		int gapTop = 5;
		int gapBot = 7;
		int totalGroupH = logoBlockH + gapTop + this.fnh_stageSize + gapBot + stackHeight;
		int groupTopY = Math.max(10, (this.height - totalGroupH) / 2);

		this.fnh_logoY = groupTopY;
		this.fnh_stageTopY = groupTopY + logoBlockH + gapTop;
		int desiredButtonsTop = this.fnh_stageTopY + this.fnh_stageSize + gapBot;

		if (minMainY != Integer.MAX_VALUE) {
			int deltaY = desiredButtonsTop - minMainY;
			if (deltaY != 0) {
				for (GuiEventListener listener : kids) {
					if (listener instanceof AbstractWidget w && w.getY() < this.height - 20) {
						w.setY(w.getY() + deltaY);
					}
				}
				bottomRowY += deltaY;
			}
		}

		for (GuiEventListener listener : kids) {
			if (listener instanceof AbstractWidget w && bottomRowY > 0 && Math.abs(w.getY() - bottomRowY) <= 4) {
				maxRightIconX = Math.max(maxRightIconX, w.getX() + w.getWidth());
			}
		}

		this.fnh_bgBtnX = maxRightIconX + 4;
		this.fnh_bgBtnY = bottomRowY > 0 ? bottomRowY : (desiredButtonsTop + stackHeight - 20);
		Button fnhBtn = Button.builder(Component.empty(), btn -> {
			if (this.minecraft != null) {
				this.minecraft.gui.setScreen(new FnHSettingsScreen((TitleScreen) (Object) this));
			}
		})
			.bounds(this.fnh_bgBtnX, this.fnh_bgBtnY, 20, 20)
			.tooltip(Tooltip.create(Component.literal("FnH UI & Background Settings")))
			.build();
		this.addRenderableWidget(fnhBtn);
	}

	@Inject(method = "extractRenderState", at = @At("HEAD"), cancellable = true)
	private void fnh_extractTitleScreen(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		this.extractPanorama(graphics, deltaTicks);

		int centerX = this.width / 2;
		int logoW = 198;
		int logoH = 31;
		int logoX = centerX - logoW / 2;
		int logoY = this.fnh_logoY;

		graphics.blit(
			RenderPipelines.GUI_TEXTURED,
			FnHBackgroundRenderer.logoTexture(),
			logoX,
			logoY,
			0.0F,
			0.0F,
			logoW,
			logoH,
			logoW,
			logoH,
			ARGB.white(1.0F)
		);

		int edW = 128;
		int edH = 16;
		int edX = centerX - edW / 2;
		int edY = logoY + logoH + 2;
		graphics.blit(
			RenderPipelines.GUI_TEXTURED,
			FnHBackgroundRenderer.editionTexture(),
			edX,
			edY,
			0.0F,
			0.0F,
			edW,
			edH,
			edW,
			edH,
			ARGB.white(1.0F)
		);

		String splash = "made by Buffer";
		float pulse = 1.0F + 0.042F * Math.abs((float) Math.sin((Util.getMillis() % 1400L) / 1400.0 * Math.PI * 2.0));
		float splashScale = 0.88F * pulse;
		graphics.pose().pushMatrix();
		graphics.pose().translate(logoX + logoW + 4.0F, logoY + logoH - 2.0F);
		graphics.pose().rotate(-0.2967F);
		graphics.pose().scale(splashScale, splashScale);
		graphics.centeredText(this.font, splash, 0, -4, 0xFFFFFFFF);
		graphics.pose().popMatrix();

		if (this.fnh_stageSize >= 28) {
			FnHSkinStageRenderer.render(
				graphics,
				this.width,
				centerX,
				this.fnh_stageTopY,
				this.fnh_stageSize,
				mouseX,
				mouseY
			);
		}

		for (Renderable renderable : ((ScreenAccessor) this).fnh_getRenderables()) {
			renderable.extractRenderState(graphics, mouseX, mouseY, deltaTicks);
		}

		drawInlineFnHIcon(graphics, this.fnh_bgBtnX, this.fnh_bgBtnY);

		String ver = "FnH 1.9.1  ·  Minecraft " + SharedConstants.getCurrentVersion().name();
		graphics.text(this.font, ver, 4, this.height - 10, 0xB8E4EAF6, true);

		ci.cancel();
	}

	@Unique
	private static void drawInlineFnHIcon(GuiGraphicsExtractor g, int bx, int by) {
		if (bx < 0 || by < 0) {
			return;
		}
		int ix = bx + 4;
		int iy = by + 4;
		g.fill(ix, iy, ix + 12, iy + 1, 0xCCDFE8FF);
		g.fill(ix, iy + 11, ix + 12, iy + 12, 0xCCDFE8FF);
		g.fill(ix, iy, ix + 1, iy + 12, 0xCCDFE8FF);
		g.fill(ix + 11, iy, ix + 12, iy + 12, 0xCCDFE8FF);
		g.fill(ix + 8, iy + 3, ix + 9, iy + 4, 0xFFFFFFFF);
		g.fill(ix + 3, iy + 4, ix + 4, iy + 5, 0xCCE6EEFF);
		g.fill(ix + 2, iy + 9, ix + 10, iy + 10, 0xFFE8EEFF);
		g.fill(ix + 3, iy + 8, ix + 9, iy + 9, 0xFFE8EEFF);
		g.fill(ix + 4, iy + 7, ix + 7, iy + 8, 0xFFFFFFFF);
		g.fill(ix + 5, iy + 6, ix + 6, iy + 7, 0xFFFFFFFF);
	}
}
