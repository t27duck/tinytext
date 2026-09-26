# tinytext

Minimal GTK4 plaintext editor in Rust. See README.md for features and usage.

## Building

There is no Rust toolchain on the host. Every cargo command runs in Docker:

    scripts/build.sh                 # release build -> dist/ (binary, LICENSE, THIRD-PARTY-LICENSES)
    scripts/build.sh build           # debug build -> target/debug/tinytext
    scripts/build.sh add <crate>     # any cargo subcommand works
    scripts/install.sh               # install dist/ to ~/.local

`CARGO_HOME` lives in `target/.cargo-home`, so crate sources are browsable at
`target/.cargo-home/registry/src/*/` when checking an API.

## Compatibility constraints

- The build image is Ubuntu 22.04 so the binary runs on glibc 2.35 / GTK 4.6
  (Ubuntu 22.04+, Debian 12+, Arch). Keep `gtk` at `features = ["v4_6"]`.
- Therefore use GTK 4.6 APIs: `MessageDialog`, `FileChooserNative`,
  `CssProvider::load_from_data`. Do NOT switch to `AlertDialog`/`FileDialog`
  (GTK 4.10+) or libadwaita.
- Unix signal handling comes from the `glib-unix` crate (glib 0.22 dropped it).
- `cargo-about` (in the image, needs `--features cli`) generates the notices;
  new dependencies must use a license listed in `about.toml` or the build fails.

## Architecture

- `main.rs`: single-instance `gtk::Application` (`dev.tinytext.TinyText`,
  `HANDLES_OPEN`). A second launch opens a window in the running process.
- `app.rs`: app actions/accelerators, registry of open editors, the
  first-launch restore prompt, opening paths, and SIGTERM/SIGINT/SIGHUP
  handling (flush snapshots and quit without prompting).
- `editor.rs`: one window (`Editor`, held as `Rc`; closures capture `Weak`).
  Header bar with hamburger menu, find bar, save/close flows.
- `session.rs`: unsaved-buffer snapshots in `$XDG_STATE_HOME/tinytext/session`
  (`<id>.txt` + optional `<id>.path`).
- `fileio.rs`: atomic writes (temp file + rename, fallback to in-place).
- `font.rs`: GNOME desktops use the GSettings monospace font, all others the
  fontconfig `monospace` alias (Omarchy's GSettings value is an unused default).
- `settings.rs`: `~/.config/tinytext/settings.conf` (word wrap only).

Snapshot invariants: each window has one snapshot id; a snapshot exists only
while the window is dirty (`is_dirty`: modified, and not an empty untitled
buffer); saving or discarding deletes it; a normal quit leaves the session dir
empty. Anything left there at startup triggers the restore prompt.

## Testing on the dev machine (Omarchy/Hyprland)

- Isolate state: `XDG_STATE_HOME=<scratch dir> ./target/debug/tinytext`.
- Make sure no installed tinytext is running first (`pgrep -x tinytext`),
  otherwise the new binary just forwards to the old process.
- Stop it with `kill -TERM $(pgrep -x tinytext)`, never `pkill -f tinytext`
  (it matches and kills the invoking shell).
- `wtype` sends keys to whatever is focused: check
  `hyprctl activewindow -j | jq -r .class` is `dev.tinytext.TinyText` (or
  `tinytext` for dialogs) before each call. It cannot trigger compositor binds.
- Trigger actions without the UI: `gdbus call --session --dest
  dev.tinytext.TinyText --object-path /dev/tinytext/TinyText --method
  org.gtk.Actions.Activate about [] {}`.
- Hyprland uses Lua dispatchers: `hyprctl dispatch 'hl.dsp.window.close()'`.
- Screenshot the active window with `grim -g "$(hyprctl activewindow -j | jq
  -r '"\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"')" out.png`.
- Portal "AccessDenied" warnings when run from a sandboxed shell are harmless.

## Conventions

- Commit early and often, one logical change per commit.
- Build everything in Docker; don't ask the user to install toolchains.
- After changing the app, rebuild and rerun `scripts/install.sh` so the
  installed copy (bound to Super+Shift+W) is current.
