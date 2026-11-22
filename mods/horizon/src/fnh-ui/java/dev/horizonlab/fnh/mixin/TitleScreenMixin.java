package dev.horizonlab.fnh.mixin;

import java.util.List;

import dev.horizonlab.fnh.FnHMenuMod;
import dev.horizonlab.fnh.FnHSettingsScreen;
import dev.horizonlab.fnh.FnHSkinStageRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Renderable;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;

@Mixin(TitleScreen.class)
public abstract class TitleScreenMixin extends Screen {
	@Unique
	private static final Identifier FNH_MC_LOGO = Identifier.fromNamespaceAndPath(
		FnHMenuMod.MOD_ID,
		"textures/gui/minecraft_logo.png"
	);

	@Unique
	private static final Identifier FNH_EDITION_BADGE = Identifier.fromNamespaceAndPath(
		FnHMenuMod.MOD_ID,
		"textures/gui/fnh_edition.png"
	);

	@Unique
	private int fnh_topButtonY = 150;

	protected TitleScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "init", at = @At("TAIL"))
	private void fnh_afterInit(CallbackInfo ci) {
		List<GuiEventListener> kids = ((ScreenAccessor) this).fnh_getChildren();

		int minMainY = Integer.MAX_VALUE;
		int maxMainBottom = 0;
		for (GuiEventListener listener : kids) {
			if (listener instanceof AbstractWidget w) {
				if (w.getWidth() >= 90 && w.getY() < this.height - 26) {
					minMainY = Math.min(minMainY, w.getY());
					maxMainBottom = Math.max(maxMainBottom, w.getY() + w.getHeight());
				}
			}
		}

		if (minMainY != Integer.MAX_VALUE) {
			int stackHeight = Math.max(72, maxMainBottom - minMainY);
			int desiredTop = Math.max(this.height - stackHeight - 24, this.height / 2 + 10);
			int deltaY = desiredTop - minMainY;
			if (deltaY != 0) {
				for (GuiEventListener listener : kids) {
					if (listener instanceof AbstractWidget w && w.getY() < this.height - 20) {
						w.setY(w.getY() + deltaY);
					}
				}
				minMainY += deltaY;
			}
			this.fnh_topButtonY = minMainY;
		} else {
			this.fnh_topButtonY = this.height / 2 + 12;
		}

		// Shift any bottom-right copyright widget slightly left so our FnH button sits cleanly in the corner
		int fnhBtnX = this.width - 46;
		for (GuiEventListener listener : kids) {
			if (listener instanceof AbstractWidget w) {
				if (w.getY() >= this.height - 22 && w.getX() + w.getWidth() > fnhBtnX - 6) {
					w.setX(Math.max(4, fnhBtnX - w.getWidth() - 8));
				}
			}
		}

		this.addRenderableWidget(
			Button.builder(Component.literal("FnH"), btn -> {
				if (this.minecraft != null) {
					this.minecraft.setScreen(new FnHSettingsScreen((TitleScreen) (Object) this));
				}
			}).bounds(fnhBtnX, this.height - 23, 40, 19).build()
		);
	}

	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void fnh_renderTitleScreen(GuiGraphics graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		this.renderPanorama(graphics, deltaTicks);

		int centerX = this.width / 2;
		int logoW = 198;
		int logoH = 31;
		int logoX = centerX - logoW / 2;
		int logoY = Math.max(8, Math.min(20, this.height / 22));

		// Official MINECRAFT logo
		graphics.blit(
			RenderPipelines.GUI_TEXTURED,
			FNH_MC_LOGO,
			logoX,
			logoY,
			0.0F,
			0.0F,
			logoW,
			logoH,
			logoW,
			logoH
		);

		// White "made by Buffer" splash
		String splash = "made by Buffer";
		float pulse = 1.0F + 0.045F * Math.abs((float) Math.sin((Util.getMillis() % 1400L) / 1400.0 * Math.PI * 2.0));
		float splashScale = 0.88F * pulse;
		graphics.pose().pushMatrix();
		graphics.pose().translate(logoX + logoW - 18.0F, logoY + logoH - 3.0F);
		graphics.pose().rotate(-0.2967F);
		graphics.pose().scale(splashScale, splashScale);
		graphics.drawCenteredString(this.font, splash, 0, -4, 0xFFFFFFFF);
		graphics.pose().popMatrix();

		// Larger chiseled [FnH Emblem] EDITION directly underneath
		int edW = 83;
		int edH = 21;
		int edX = centerX - edW / 2;
		int edY = logoY + logoH + 2;
		graphics.blit(
			RenderPipelines.GUI_TEXTURED,
			FNH_EDITION_BADGE,
			edX,
			edY,
			0.0F,
			0.0F,
			edW,
			edH,
			edW,
			edH
		);

		// 3D Walk-In Player Skin Stage (fits cleanly above Singleplayer with zero clipping)
		int stageTop = edY + edH + 2;
		int stageAvail = (this.fnh_topButtonY - 3) - stageTop;
		int stageSize = Math.max(0, Math.min(110, stageAvail));
		if (stageSize >= 28) {
			int centeredStageTop = stageTop + (stageAvail - stageSize) / 2;
			FnHSkinStageRenderer.render(
				graphics,
				this.width,
				centerX,
				centeredStageTop,
				stageSize,
				mouseX,
				mouseY
			);
		}

		// Render all Dark-Glass menu buttons + ModMenu Mods button + FnH corner button
		for (Renderable renderable : ((ScreenAccessor) this).fnh_getRenderables()) {
			renderable.render(graphics, mouseX, mouseY, deltaTicks);
		}

		graphics.drawString(this.font, "Minecraft 1.21.11/Fabric (FnH 1.8)", 6, this.height - 13, 0x8CFFFFFF);
		ci.cancel();
	}
}
