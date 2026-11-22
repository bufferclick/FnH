package dev.horizonlab.fnh.mixin;

import dev.horizonlab.fnh.FnHSettingsScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.network.chat.Component;

@Mixin(OptionsScreen.class)
public abstract class OptionsScreenMixin extends Screen {
	protected OptionsScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "init", at = @At("TAIL"))
	private void fnh_addOptionsCornerButton(CallbackInfo ci) {
		this.addRenderableWidget(
			Button.builder(Component.literal("FnH"), btn -> {
				if (this.minecraft != null) {
					this.minecraft.setScreen(new FnHSettingsScreen((OptionsScreen) (Object) this));
				}
			}).bounds(this.width - 46, 6, 40, 19).build()
		);
	}
}
