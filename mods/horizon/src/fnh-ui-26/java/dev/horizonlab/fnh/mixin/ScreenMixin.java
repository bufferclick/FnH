package dev.horizonlab.fnh.mixin;

import dev.horizonlab.fnh.FnHBackgroundRenderer;
import dev.horizonlab.fnh.FnHGlassStyle;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.gui.GuiGraphicsExtractor;
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

	@Inject(method = "extractRenderStateWithTooltipAndSubtitles", at = @At("HEAD"))
	private void fnh_onFrameStart(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		FnHGlassStyle.beginScreenFrame(self, mouseX, mouseY);
		this.fnh_pushedSlide = false;
	}

	@Inject(
		method = "extractRenderStateWithTooltipAndSubtitles",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screens/Screen;extractBackground(Lnet/minecraft/client/gui/GuiGraphicsExtractor;IIF)V",
			shift = At.Shift.AFTER
		)
	)
	private void fnh_afterBackground(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		if (FnHGlassStyle.isInGame() || self instanceof TitleScreen || self instanceof AbstractContainerScreen<?>) {
			return;
		}
		float slideY = FnHGlassStyle.currentOpenSlideUpY(self);
		if (Math.abs(slideY) > 0.15F) {
			graphics.pose().pushMatrix();
			graphics.pose().translate(0.0F, slideY);
			this.fnh_pushedSlide = true;
		}
	}

	@Inject(method = "extractRenderStateWithTooltipAndSubtitles", at = @At("RETURN"))
	private void fnh_onFrameEnd(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float deltaTicks, CallbackInfo ci) {
		if (this.fnh_pushedSlide) {
			graphics.pose().popMatrix();
			this.fnh_pushedSlide = false;
		}
	}

	@Inject(method = "extractPanorama", at = @At("HEAD"), cancellable = true)
	private void fnh_extractPanorama(GuiGraphicsExtractor graphics, float deltaTicks, CallbackInfo ci) {
		Screen self = (Screen) (Object) this;
		FnHBackgroundRenderer.render(graphics, this.width, this.height, self instanceof TitleScreen);
		ci.cancel();
	}
}
