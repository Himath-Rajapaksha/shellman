> **Fork:** [Himath-Rajapaksha/shellman](https://github.com/Himath-Rajapaksha/shellman),
> maintained by Himath Rajapaksha. Based on
> [scresto09/shellman](https://github.com/scresto09/shellman) v0.1.0.
> GPL-3.0-or-later. This fork is source-only for v0.2; see [FORK.md](FORK.md).

# ShellMan

A terminal at the bottom of Nautilus (GNOME Files) windows.
Home page: https://scresto.pages-perso.free.fr/shellman/

![ShellMan in Nautilus](https://scresto.pages-perso.free.fr/shellman/img/screenshot.png)

- **F4** shows the terminal, gives it the keyboard, then hides it.
- The shell **follows the displayed folder** (bash, zsh, fish), without typing
  anything into it.
- The **selected files** are in `$SEL`: `cp "${SEL[@]}" /tmp`.
  **Ctrl+Shift+S** inserts their paths, quoted.
- **One terminal per tab**, which can be split (**Alt+Shift+=**, **Alt+Shift+-**).
- The **colors of your terminal** (GNOME Terminal or Ptyxis), or a palette.
- Shortcuts, colors and font can be changed in **Terminal ▾ › Preferences…**.

Tested with Nautilus 46 to 51. ShellMan changes Nautilus' window, which the
extension API does not really allow: a Nautilus update can break it.

## Installation

Each [release](https://github.com/scresto09/shellman/releases) has a `.deb`
(Ubuntu 24.04 and later, Debian 13) and a `.rpm` (Fedora, RHEL 10, openSUSE).
This fork does not publish packages in v0.2; build from source instead.

```bash
sudo apt install ./shellman_*.deb                          # Debian, Ubuntu
sudo dnf install ./shellman-*.rpm                          # Fedora, RHEL
sudo zypper install --allow-unsigned-rpm ./shellman-*.rpm  # openSUSE
nautilus -q
```

From source (Rust, GTK 4, libadwaita and the Nautilus extension headers):

```bash
make && sudo make install
```

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).
