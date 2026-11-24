package dev.horizonlab.fnh.mixin;

import dev.horizonlab.fnh.FnHGlassStyle;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;

@Mixin(Minecraft.class)
public abstract class MinecraftMixin {
	@Shadow
	public Screen screen;

	@Inject(method = "setScreen", at = @At("HEAD"))
	private void fnh_beforeSetScreen(Screen nextScreen, CallbackInfo ci) {
		FnHGlassStyle.onSetScreen(this.screen, nextScreen);
	}

	@Inject(method = "tick", at = @At("RETURN"))
	private void fnh_afterTick(CallbackInfo ci) {
		FnHGlassStyle.tickInGameMotionBlur();
	}
}
