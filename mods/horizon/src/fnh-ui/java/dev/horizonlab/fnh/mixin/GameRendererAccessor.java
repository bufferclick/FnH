package dev.horizonlab.fnh.mixin;

import com.mojang.blaze3d.resource.CrossFrameResourcePool;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

import net.minecraft.client.renderer.GameRenderer;

@Mixin(GameRenderer.class)
public interface GameRendererAccessor {
	@Accessor("resourcePool")
	CrossFrameResourcePool fnh_getResourcePool();
}
