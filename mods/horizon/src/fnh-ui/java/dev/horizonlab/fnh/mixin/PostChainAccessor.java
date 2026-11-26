package dev.horizonlab.fnh.mixin;

import java.util.Map;

import com.mojang.blaze3d.pipeline.RenderTarget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

import net.minecraft.client.renderer.PostChain;
import net.minecraft.resources.Identifier;

@Mixin(PostChain.class)
public interface PostChainAccessor {
	@Accessor("persistentTargets")
	Map<Identifier, RenderTarget> fnh_getPersistentTargets();
}
