package dev.horizonlab.fnh.mixin;

import com.mojang.blaze3d.resource.GraphicsResourceAllocator;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import dev.horizonlab.fnh.FnHGlassStyle;
import org.joml.Vector4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.state.level.CameraRenderState;

@Mixin(LevelRenderer.class)
public abstract class LevelRendererMixin {
	@Inject(method = "render", at = @At("RETURN"))
	private void fnh_afterWorldRender(
		GraphicsResourceAllocator resourceAllocator,
		boolean renderOutline,
		CameraRenderState cameraState,
		GpuBufferSlice terrainFog,
		Vector4f fogColor,
		boolean shouldRenderSky,
		boolean consistentDepthRequired,
		CallbackInfo ci
	) {
		FnHGlassStyle.applyInGameMotionBlur(resourceAllocator);
	}
}
