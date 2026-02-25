use smithay::input::keyboard::XkbConfig;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct KeyboardConfig {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
}

impl KeyboardConfig {
    pub fn to_xkb_config(&self) -> XkbConfig<'_> {
        XkbConfig {
            rules: &self.rules,
            model: &self.model,
            layout: &self.layout,
            variant: &self.variant,
            options: self.options.clone(),
        }
    }
}

fn run_cmd_output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn parse_first_xkb_source(sources: &str) -> Option<(String, String)> {
    let marker = "'xkb',";
    let marker_pos = sources.find(marker)?;
    let after = &sources[marker_pos + marker.len()..];
    let first_quote = after.find('\'')?;
    let rest = &after[first_quote + 1..];
    let second_quote = rest.find('\'')?;
    let source = &rest[..second_quote];
    let (layout, variant) = match source.split_once('+') {
        Some((l, v)) => (l.to_string(), v.to_string()),
        None => (source.to_string(), String::new()),
    };
    Some((layout, variant))
}

fn parse_xkb_options(options: &str) -> Option<String> {
    let values: Vec<String> = options
        .split('\'')
        .enumerate()
        .filter_map(|(idx, part)| {
            if idx % 2 == 1 && !part.is_empty() {
                Some(part.to_string())
            } else {
                None
            }
        })
        .collect();

    if values.is_empty() {
        None
    } else {
        Some(values.join(","))
    }
}

pub fn keyboard_config_from_system() -> KeyboardConfig {
    let mut cfg = KeyboardConfig::default();

    if let Some(sources) = run_cmd_output(
        "gsettings",
        &["get", "org.gnome.desktop.input-sources", "sources"],
    ) {
        if let Some((layout, variant)) = parse_first_xkb_source(&sources) {
            cfg.layout = layout;
            cfg.variant = variant;
        }
    }

    if let Some(options) = run_cmd_output(
        "gsettings",
        &["get", "org.gnome.desktop.input-sources", "xkb-options"],
    ) {
        cfg.options = parse_xkb_options(&options);
    }

    cfg
}
