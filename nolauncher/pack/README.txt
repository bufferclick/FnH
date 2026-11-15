NoLauncherHorizon v@VERSION@
==========================

Horizon, with no launcher.

Double-click NoLauncherHorizon.exe. Your browser opens on Microsoft's sign-in: the code is
already on your clipboard, paste it with Ctrl+V and sign in. The tab behind it shows what is
happening — the newest Horizon build being fetched, the game's files being prepared from your
own Minecraft — and then Horizon starts by itself.

There is nothing to install and nothing to set up. The first start takes a few minutes on a
computer that has never played Horizon (it downloads a build and prepares the game's files);
after that it goes straight into the game, and a start with nothing new to do opens no browser
at all.

You need a Microsoft account that owns Minecraft: Java Edition, and Java 25 or a Minecraft
install of your own (the official launcher, Prism, MultiMC, the Modrinth App, ATLauncher or
GDLauncher) for the first preparation. Worlds, options and sign-in are the same ones the Horizon
launcher uses, so the two can live side by side.

Where things are (Windows):

  %APPDATA%\horizon-launcher\nolauncher\nolauncher.log    what happened, line by line
  %APPDATA%\horizon-launcher\nolauncher\nolauncher.html   the page the browser showed
  %APPDATA%\horizon-launcher\horizon                      your worlds and options

If it does not start, the page it opened says why; the log says more. Starting it twice while a
game runs starts a second game, exactly as the launcher allows.

Options: NoLauncherHorizon.exe --tag latest --no-update --version --help
Anything else goes to the game: NoLauncherHorizon.exe --world MyWorld

MIT licence. Not an official Minecraft product; not approved by or associated with Mojang or
Microsoft. Horizon is made by the Horizon Team; NoLauncherHorizon is Buffer's build of it with
the launcher removed.
