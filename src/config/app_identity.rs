use gtk4::gio;
use gtk4::prelude::{AppInfoExt, Cast};
use std::path::Path;

fn themed_icon_name(icon: gio::Icon) -> Option<String> {
    icon.dynamic_cast::<gio::ThemedIcon>()
        .ok()
        .and_then(|themed| themed.names().into_iter().next())
        .map(|name| name.to_string())
}

fn lookup_desktop_id(desktop_id: &str) -> Option<(String, Option<String>)> {
    let app_info = gio::DesktopAppInfo::new(desktop_id)?;
    let title = app_info.display_name().to_string();
    let icon_name = app_info.icon().and_then(themed_icon_name);
    Some((title, icon_name))
}

fn desktop_candidates(raw: &str) -> Vec<String> {
    let lower = raw.to_lowercase();
    let mut out = Vec::new();

    if raw.ends_with(".desktop") {
        out.push(raw.to_string());
    } else if !raw.is_empty() {
        out.push(raw.to_string());
        out.push(format!("{raw}.desktop"));
    }

    if !lower.is_empty() {
        if lower.ends_with(".desktop") {
            if !out.iter().any(|s| s == &lower) {
                out.push(lower.clone());
            }
        } else {
            if !out.iter().any(|s| s == &lower) {
                out.push(lower.clone());
            }
            let lowered_desktop = format!("{lower}.desktop");
            if !out.iter().any(|s| s == &lowered_desktop) {
                out.push(lowered_desktop);
            }
        }
    }

    out
}

fn basename(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command)
        .to_string()
}

pub fn launch_seed(command: &str) -> (String, Option<String>, String) {
    let base = basename(command);
    for candidate in desktop_candidates(&base) {
        if let Some((title, icon)) = lookup_desktop_id(&candidate) {
            return (title, icon, base);
        }
    }
    (base.clone(), None, base)
}

pub fn metadata_title_icon(
    title: Option<String>,
    app_id: Option<String>,
) -> (String, Option<String>) {
    let raw_id = app_id.unwrap_or_default();
    let raw_title = title.unwrap_or_default();

    for candidate in desktop_candidates(&raw_id) {
        if let Some((desktop_title, icon)) = lookup_desktop_id(&candidate) {
            let effective_title = if raw_title.is_empty() {
                desktop_title
            } else {
                raw_title
            };
            return (effective_title, icon);
        }
    }

    if !raw_title.is_empty() {
        return (raw_title, None);
    }
    if !raw_id.is_empty() {
        return (raw_id, None);
    }
    ("WayFrame Container".to_string(), None)
}
