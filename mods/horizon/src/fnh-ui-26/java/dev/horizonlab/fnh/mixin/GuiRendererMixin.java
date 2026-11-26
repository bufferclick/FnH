package dev.horizonlab.fnh.mixin;

import dev.horizonlab.fnh.FnHGlassStyle;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.gui.render.GuiRenderer;

@Mixin(GuiRenderer.class)
public abstract class GuiRendererMixin {
	@Inject(method = "render", at = @At("RETURN"), require = 0)
	private void fnh_afterGuiRender(CallbackInfo ci) {
		FnHGlassStyle.applyUiMotionBlur();
	}
}
