package dev.horizonlab.fnh.mixin;

import com.mojang.blaze3d.pipeline.RenderPipeline;
import dev.horizonlab.fnh.FnHGlassStyle;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.resources.Identifier;

@Mixin(GuiGraphics.class)
public abstract class GuiGraphicsMixin {
	@Inject(
		method = "blitSprite(Lcom/mojang/blaze3d/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIII)V",
		at = @At("HEAD"),
		cancellable = true
	)
	private void fnh_blitSprite6(
		RenderPipeline pipeline,
		Identifier sprite,
		int x,
		int y,
		int width,
		int height,
		CallbackInfo ci
	) {
		GuiGraphics self = (GuiGraphics) (Object) this;
		if (FnHGlassStyle.tryRenderGlassWidget(self, sprite, x, y, width, height, 0xFFFFFFFF, false)) {
			ci.cancel();
		}
	}

	@Inject(
		method = "blitSprite(Lcom/mojang/blaze3d/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIII)V",
		at = @At("HEAD"),
		cancellable = true
	)
	private void fnh_blitSprite7(
		RenderPipeline pipeline,
		Identifier sprite,
		int x,
		int y,
		int width,
		int height,
		int color,
		CallbackInfo ci
	) {
		GuiGraphics self = (GuiGraphics) (Object) this;
		if (FnHGlassStyle.tryRenderGlassWidget(self, sprite, x, y, width, height, color, true)) {
			ci.cancel();
		}
	}

	@ModifyVariable(
		method = "drawString(Lnet/minecraft/client/gui/Font;Lnet/minecraft/util/FormattedCharSequence;IIIZ)V",
		at = @At("HEAD"),
		ordinal = 2,
		argsOnly = true
	)
	private int fnh_upgradeSeqLabelColor(int color) {
		return FnHGlassStyle.upgradeContainerLabelColor(color);
	}

	@ModifyVariable(
		method = "drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;IIIZ)V",
		at = @At("HEAD"),
		ordinal = 2,
		argsOnly = true
	)
	private int fnh_upgradeStrLabelColor(int color) {
		return FnHGlassStyle.upgradeContainerLabelColor(color);
	}
}
