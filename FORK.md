# Fork

ShellMan is a Nautilus extension that embeds a terminal at the bottom of
Nautilus (GNOME Files) windows.

## Upstream and fork

- Upstream: [scresto09/shellman](https://github.com/scresto09/shellman), v0.1.0.
- Fork: [Himath-Rajapaksha/shellman](https://github.com/Himath-Rajapaksha/shellman),
  maintained by Himath Rajapaksha.
- Fork date: 2026-11-13.

## License

GPL-3.0-or-later. Upstream copyright is held by Sylvain Cresto; fork
modifications are copyright Himath Rajapaksha. The license is unchanged from
upstream, and `gtk::License::Gpl30` in the About dialog means
GPL-3.0-or-later.

## What the fork changes

The v0.2 release turns the terminal into a file-aware terminal: a context menu,
drag-and-drop of files into the terminal, a status line, and new/duplicate
terminal actions.

## Installing the fork

The upstream and fork binaries must not be installed at the same time: both
inject a panel into Nautilus, and the two panels would conflict. Remove one
before installing the other.

v0.2 is source-only. No `.deb` or `.rpm` packages are published for this fork;
build and install from source instead (see the README).

## Syncing with upstream

Rebase feature branches on `upstream/main`. Never merge upstream into a feature
branch without a review.
