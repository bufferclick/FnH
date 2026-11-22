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
		if (self instanceof TitleScreen) {
			return;
		}
		float slideY = FnHGlassStyle.currentSlideOffsetY();
		if (slideY > 0.05F) {
			graphics.pose().pushMatrix();
			graphics.pose().translate(0.0F, slideY);
			this.fnh_pushedSlide = true;
		}
	}

	@Inject(method = "renderWithTooltipAndSubtitles", at = @At("RETURN"))
	private void fnh_onFrameEnd(GuiGraphics graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		if (this.fnh_pushedSlide) {
			graphics.pose().popMatrix();
			this.fnh_pushedSlide = false;
		}
	}

	@Inject(method = "renderPanorama", at = @At("HEAD"), cancellable = true)
	private void fnh_renderPanorama(GuiGraphics graphics, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		FnHBackgroundRenderer.render(graphics, this.width, this.height, self instanceof TitleScreen);
		ci.cancel();
	}
}
