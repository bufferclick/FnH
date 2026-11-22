package dev.horizonlab.fnh.mixin;

import dev.horizonlab.fnh.FnHBackgroundRenderer;
import dev.horizonlab.fnh.FnHGlassStyle;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;

@Mixin(Screen.class)
public abstract class ScreenMixin {
	@Shadow
	public int width;

	@Shadow
	public int height;

	@Unique
	private boolean fnh_pushedSlide = false;

	@Inject(method = "renderWithTooltipAndSubtitles", at = @At("HEAD"))
	private void fnh_onFrameStart(GuiGraphics graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		FnHGlassStyle.beginScreenFrame(self, mouseX, mouseY);
		this.fnh_pushedSlide = false;
	}

	@Inject(
		method = "renderWithTooltipAndSubtitles",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screens/Screen;renderBackground(Lnet/minecraft/client/gui/GuiGraphics;IIF)V",
			shift = At.Shift.AFTER
		)
	)
	private void fnh_afterBackground(GuiGraphics graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		// Strictly NEVER animate in-game screens or container items (`/ah`, inventory, chests)
		if (FnHGlassStyle.isInGame() || self instanceof TitleScreen || self instanceof AbstractContainerScreen<?>) {
			return;
		}
		float slideUpY = FnHGlassStyle.currentOpenSlideUpY(self);
		if (slideUpY > 0.25F) {
			graphics.pose().pushMatrix();
			graphics.pose().translate(0.0F, slideUpY);
			this.fnh_pushedSlide = true;
			float speed01 = Math.min(1.0F, slideUpY / 42.0F);
			FnHGlassStyle.renderMenuMotionBlur(graphics, self, speed01, 1.0F);
		}
	}

	@Inject(method = "renderWithTooltipAndSubtitles", at = @At("RETURN"))
	private void fnh_onFrameEnd(GuiGraphics graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		if (this.fnh_pushedSlide) {
			graphics.pose().popMatrix();
			this.fnh_pushedSlide = false;
		}
		// Render any closing front-end menu sliding DOWN with vertical motion blur!
		if (!FnHGlassStyle.isInGame()) {
			FnHGlassStyle.renderClosingMenuOverlay(graphics, deltaTicks);
		}
	}

	@Inject(method = "renderPanorama", at = @At("HEAD"), cancellable = true)
	private void fnh_renderPanorama(GuiGraphics graphics, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		FnHBackgroundRenderer.render(graphics, this.width, this.height, self instanceof TitleScreen);
		ci.cancel();
	}
}
