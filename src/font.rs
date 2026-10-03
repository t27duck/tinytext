use gtk::prelude::*;
use gtk::{gdk, gio, pango};

const DEFAULT_SIZE: f64 = 11.0;

/// Installs a stylesheet that renders editor text in the system's monospace font.
pub fn install() {
    let (family, size) = system_monospace();
    let family = family.replace(['"', '\\'], "");
    let css = format!(".tinytext-editor {{ font-family: \"{family}\"; font-size: {size}pt; }}");

    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// GNOME-family desktops configure the monospace font through GSettings.
/// Everywhere else (Hyprland/Omarchy, KDE, ...) the fontconfig `monospace`
/// alias is the source of truth.
fn system_monospace() -> (String, f64) {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let gnome_like = desktop
        .split(':')
        .any(|d| matches!(d, "gnome" | "gnome-classic" | "ubuntu" | "unity" | "budgie" | "pantheon"));

    gnome_like
        .then(gnome_monospace)
        .flatten()
        .unwrap_or_else(|| ("monospace".to_string(), DEFAULT_SIZE))
}

fn gnome_monospace() -> Option<(String, f64)> {
    const SCHEMA: &str = "org.gnome.desktop.interface";
    const KEY: &str = "monospace-font-name";

    let schema = gio::SettingsSchemaSource::default()?.lookup(SCHEMA, true)?;
    if !schema.has_key(KEY) {
        return None;
    }
    let desc = pango::FontDescription::from_string(&gio::Settings::new(SCHEMA).string(KEY));
    let family = desc.family()?.to_string();
    let size = if desc.size() > 0 {
        desc.size() as f64 / pango::SCALE as f64
    } else {
        DEFAULT_SIZE
    };
    Some((family, size))
}
