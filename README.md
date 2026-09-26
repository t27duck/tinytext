# tinytext

A tiny plaintext editor for Linux, built with Rust and GTK4. It runs on
Arch/Omarchy and Debian 12+/Ubuntu 22.04+.

## Features

- No menubar: a single hamburger menu (also opened with F10)
- One window per file; unsaved windows survive crashes, logouts and reboots
  and are offered for restore on the next launch
- Closing a modified window asks to save or discard
- Uses the system monospace font (GNOME's setting on GNOME desktops,
  fontconfig's `monospace` alias elsewhere)

| Shortcut | Action |
| --- | --- |
| Ctrl+N | New window |
| Ctrl+O | Open file |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save as |
| Ctrl+F | Find (Enter / Shift+Enter for next / previous) |
| Ctrl+W | Close window (the compositor's close shortcut works too) |
| Ctrl+Z / Ctrl+Shift+Z | Undo / redo |

## Build

The only requirement is Docker:

    scripts/build.sh            # builds dist/tinytext
    scripts/install.sh          # installs to ~/.local (binary + launcher entry)
    scripts/install.sh --set-default   # ...and make it the default for .txt/.md files
    scripts/install.sh --uninstall     # also removes defaults pointing at tinytext

`scripts/build.sh` also passes arbitrary cargo commands through, e.g.
`scripts/build.sh build` for a debug build.

The binary is built on Ubuntu 22.04 and links against the system's GTK 4
(4.6 or newer), which is installed by default on GNOME-based systems and
Omarchy. On minimal systems install `gtk4` (Arch) or `libgtk-4-1`
(Debian/Ubuntu).

## Files

- Unsaved buffers: `$XDG_STATE_HOME/tinytext/session` (default `~/.local/state`)
- Settings: `$XDG_CONFIG_HOME/tinytext/settings.conf`
