package dev.horizonlab.fnh;

import java.nio.file.Path;
import java.util.List;

import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.AbstractSliderButton;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;
import net.minecraft.util.Util;

public final class FnHSettingsScreen extends Screen {
	private final Screen parent;
	private EditBox skinUserBox;

	public FnHSettingsScreen(Screen parent) {
		super(Component.literal("FnH"));
		this.parent = parent;
	}

	@Override
	protected void init() {
		int colLeft = this.width / 2 - 154;
		int colRight = this.width / 2 + 4;
		int y = 44;

		// Row 1: Background switcher + Open Backgrounds Folder
		this.addRenderableWidget(Button.builder(currentBgLabel(), btn -> {
			cycleBackground();
			btn.setMessage(currentBgLabel());
		}).bounds(colLeft, y, 150, 20).build());

		this.addRenderableWidget(Button.builder(Component.literal("Open Backgrounds Folder"), btn -> {
			openFolderSafe(FnHConfig.backgroundsDir());
		}).bounds(colRight, y, 150, 20).build());

		y += 24;

		// Row 2: Background Dim slider + Motion Blur slider
		this.addRenderableWidget(new AbstractSliderButton(
			colLeft, y, 150, 20,
			Component.literal("Background Dim: " + FnHConfig.bgDimPercent + "%"),
			FnHConfig.bgDimPercent / 65.0
		) {
			@Override
			protected void updateMessage() {
				this.setMessage(Component.literal("Background Dim: " + FnHConfig.bgDimPercent + "%"));
			}

			@Override
			protected void applyValue() {
				FnHConfig.bgDimPercent = (int) Math.round(this.value * 65.0);
				FnHConfig.save();
			}
		});

		this.addRenderableWidget(new AbstractSliderButton(
			colRight, y, 150, 20,
			Component.literal("Motion Blur: " + FnHConfig.motionBlurPercent + "%"),
			FnHConfig.motionBlurPercent / 150.0
		) {
			@Override
			protected void updateMessage() {
				this.setMessage(Component.literal("Motion Blur: " + FnHConfig.motionBlurPercent + "%"));
			}

			@Override
			protected void applyValue() {
				FnHConfig.motionBlurPercent = (int) Math.round(this.value * 150.0);
				FnHConfig.save();
			}
		});

		y += 24;

		// Row 3: 3D Player Skin toggle + Replay Walk-In Intro
		this.addRenderableWidget(Button.builder(skinToggleLabel(), btn -> {
			FnHConfig.showPlayerSkin = !FnHConfig.showPlayerSkin;
			FnHConfig.save();
			btn.setMessage(skinToggleLabel());
		}).bounds(colLeft, y, 150, 20).build());

		this.addRenderableWidget(Button.builder(Component.literal("Replay Walk-In Intro"), btn -> {
			FnHSkinStageRenderer.triggerWalkIn();
			if (this.minecraft != null) {
				this.minecraft.setScreen(this.parent);
			}
		}).bounds(colRight, y, 150, 20).build());

		y += 28;

		// Row 4: Skin Override Username EditBox + Apply button
		this.skinUserBox = new EditBox(
			this.font,
			colLeft,
			y,
			220,
			20,
			Component.literal("Skin Username")
		);
		this.skinUserBox.setMaxLength(32);
		this.skinUserBox.setValue(FnHConfig.skinOverrideUser == null ? "" : FnHConfig.skinOverrideUser);
		this.skinUserBox.setHint(
			Component.literal("Account skin (or type username)")
				.withStyle(Style.EMPTY.withColor(ChatFormatting.GRAY))
		);
		this.addRenderableWidget(this.skinUserBox);

		this.addRenderableWidget(Button.builder(Component.literal("Load Skin"), btn -> {
			FnHConfig.skinOverrideUser = this.skinUserBox.getValue().trim();
			FnHConfig.save();
			FnHSkinStageRenderer.forceReloadSkin();
		}).bounds(colLeft + 226, y, 82, 20).build());

		// Bottom Done button
		this.addRenderableWidget(Button.builder(Component.literal("Done"), btn -> {
			FnHConfig.save();
			if (this.minecraft != null) {
				this.minecraft.setScreen(this.parent);
			}
		}).bounds(this.width / 2 - 100, this.height - 28, 200, 20).build());
	}

	private static void openFolderSafe(Path dir) {
		try {
			Util.getPlatform().openPath(dir);
			return;
		} catch (Throwable ignored) {
		}
		try {
			if (java.awt.Desktop.isDesktopSupported()) {
				java.awt.Desktop.getDesktop().open(dir.toFile());
				return;
			}
		} catch (Throwable ignored) {
		}
		try {
			new ProcessBuilder("explorer.exe", dir.toAbsolutePath().toString()).start();
		} catch (Throwable ignored) {
		}
	}

	private void cycleBackground() {
		List<Path> custom = FnHConfig.availableCustomBackgrounds();
		if (custom.isEmpty()) {
			FnHConfig.customBgFile = "";
		} else {
			Path currentResolved = FnHConfig.resolveActiveBackgroundPath();
			if (currentResolved == null) {
				FnHConfig.customBgFile = custom.get(0).getFileName().toString();
			} else {
				int idx = -1;
				String activeName = currentResolved.getFileName().toString();
				for (int i = 0; i < custom.size(); i++) {
					if (custom.get(i).getFileName().toString().equalsIgnoreCase(activeName)) {
						idx = i;
						break;
					}
				}
				if (idx < 0 || idx + 1 >= custom.size()) {
					FnHConfig.customBgFile = FnHConfig.DEFAULT_SENTINEL;
				} else {
					FnHConfig.customBgFile = custom.get(idx + 1).getFileName().toString();
				}
			}
		}
		FnHConfig.save();
		FnHBackgroundRenderer.reload();
	}

	private static Component currentBgLabel() {
		Path resolved = FnHConfig.resolveActiveBackgroundPath();
		if (resolved == null) {
			return Component.literal("BG: Default Milky Way");
		}
		String name = resolved.getFileName().toString();
		String shortName = name.length() > 16 ? name.substring(0, 14) + ".." : name;
		return Component.literal("BG: " + shortName);
	}

	private static Component skinToggleLabel() {
		return Component.literal("3D Player Skin: " + (FnHConfig.showPlayerSkin ? "ON" : "OFF"));
	}

	@Override
	public void onClose() {
		FnHConfig.save();
		if (this.minecraft != null) {
			this.minecraft.setScreen(this.parent);
		}
	}

	@Override
	public void render(GuiGraphics graphics, int mouseX, int mouseY, float deltaTicks) {
		super.render(graphics, mouseX, mouseY, deltaTicks);
		graphics.drawCenteredString(this.font, this.title, this.width / 2, 14, 0xFFFFFFFF);
		graphics.fill(0, 32, this.width, 33, 0x2EFFFFFF);
		graphics.fill(0, this.height - 36, this.width, this.height - 35, 0x2EFFFFFF);
	}
}
