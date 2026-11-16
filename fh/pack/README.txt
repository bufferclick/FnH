Fabric n Horizon v@VERSION@
===========================

Your Horizon instances, one window.

Unzip this folder anywhere and double-click FH.exe. A window opens — JLac's window, drawn the way
JLac draws its own: the bar across the top with the chosen instance's mark, its name and one lime
Play; the instances down the left in a darker pane; the instance's own pages on the right
(Overview, Mods, Worlds, Logs); and a status bar along the bottom that says what is happening.

Press Play. If it has to, FH signs you in with Microsoft: the code appears in the window, goes on
your clipboard, and Microsoft's page opens once with it already filled in. It installs the build
the instance plays when this computer has none, takes the game's files from your own Minecraft the
way the launcher does, and starts the game — signed in, so joining a server works first time.

New instance makes another: a Horizon instance of its own, or a Minecraft: Java Edition one —
vanilla, or with Fabric (its loader version comes with the first Play). The Mods page lists the
jars in the instance's mods folder, Worlds its saves, and Logs the end of its latest log.

It uses exactly the instances the Horizon launcher has — the same folder, the same worlds and
options, the same sign-in — so FH and the launcher can live side by side.

Signing in needs no account of yours beyond Microsoft's: there is no app to register and nothing
to configure.

Where things are (Windows):

  %APPDATA%\horizon-launcher\fh\fh.log                what happened, line by line

  %APPDATA%\horizon-launcher\instances.json           the instances (shared with the launcher)
  %APPDATA%\horizon-launcher\horizon                  your worlds and options

If a launch does not start, the status bar says why and the log says more.

Options: FH.exe --version --help

MIT licence. Not an official Minecraft product; not approved by or associated with Mojang or
Microsoft. Horizon is made by the Horizon Team; Fabric n Horizon is Buffer's window over it,
styled after JLac's own.
