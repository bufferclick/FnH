package dev.horizonlab.fnh;

import net.fabricmc.api.ClientModInitializer;

public final class FnHMenuMod implements ClientModInitializer {
	public static final String MOD_ID = "fnh_ui";

	@Override
	public void onInitializeClient() {
		FnHConfig.load();
	}
}
