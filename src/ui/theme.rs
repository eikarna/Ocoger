//! Embedded theme palettes (Phase 4 polish).
//!
//! Each theme exposes semantic slots: `bg`, `fg`, `accent`, `dim`,
//! `highlight_bg`, `warn`, `border`, `syntax_keyword`. Widgets read
//! `App.theme.<slot>` instead of hard-coded `Color::*` constants.
//!
//! Ships 7 presets matching the upstream OpenCode desktop palettes:
//! `opencode` (default), `monokai`, `github-dark`, `dracula` (classic),
//! `nord`, `ayu-dark`, `vercel-dark`. Users can register their own under
//! `.ocoger/themes/<name>.toml` — same slot names — and reference it from
//! `opencode.jsonc`'s `theme` key.
//!
//! Palette hex sources are documented inline per theme.

use ratatui::style::Color;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Logo / primary text.
    pub fg: Color,
    /// Suggested background (some terminals ignore it; still the right pick for borders).
    pub bg: Color,
    /// Primary action / brand color.
    pub accent: Color,
    /// Secondary (weak) text.
    pub dim: Color,
    /// Cursor-line highlight background.
    pub highlight_bg: Color,
    /// Warnings / errors.
    pub warn: Color,
    /// Border stroke (panels + modal outlines).
    pub border: Color,
    /// Syntax accent for diff / code highlights (not extensively used yet).
    pub syntax_keyword: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::opencode()
    }
}

impl Theme {
    pub fn name(&self) -> &'static str {
        // We can't rely on tagging the struct; use a static mapping table
        // derived from palette fingerprints.
        let sig: (Color, Color) = (self.fg, self.accent);
        for (name, t) in BUILTINS.iter() {
            if (t.fg, t.accent) == sig {
                return name;
            }
        }
        "unknown"
    }

    pub fn opencode() -> Self {
        // https://github.com/sst/opencode/blob/dev/packages/ui/src/theme/themes/opencode.json
        Self {
            fg: Color::from_u32(0xeeeeee),
            bg: Color::from_u32(0x0a0a0a),
            accent: Color::from_u32(0xfab283),
            dim: Color::from_u32(0x808080),
            highlight_bg: Color::from_u32(0x282828),
            warn: Color::from_u32(0xe06c75),
            border: Color::from_u32(0x3c3c3c),
            syntax_keyword: Color::from_u32(0x5c9cf5),
        }
    }

    pub fn monokai() -> Self {
        // https://github.com/sst/opencode/blob/dev/packages/ui/src/theme/themes/monokai.json
        Self {
            fg: Color::from_u32(0xf8f8f2),
            bg: Color::from_u32(0x272822),
            accent: Color::from_u32(0xae81ff),
            dim: Color::from_u32(0x75715e),
            highlight_bg: Color::from_u32(0x3e3d32),
            warn: Color::from_u32(0xf92672),
            border: Color::from_u32(0x49483e),
            syntax_keyword: Color::from_u32(0xf92672),
        }
    }

    pub fn github_dark() -> Self {
        // https://github.com/sst/opencode/blob/dev/packages/ui/src/theme/themes/github.json
        Self {
            fg: Color::from_u32(0xc9d1d9),
            bg: Color::from_u32(0x0d1117),
            accent: Color::from_u32(0x58a6ff),
            dim: Color::from_u32(0x8b949e),
            highlight_bg: Color::from_u32(0x1f6feb),
            warn: Color::from_u32(0xf85149),
            border: Color::from_u32(0x30363d),
            syntax_keyword: Color::from_u32(0xff7b72),
        }
    }

    pub fn dracula() -> Self {
        // Classic Dracula (https://draculatheme.com/contribute)
        Self {
            fg: Color::from_u32(0xf8f8f2),
            bg: Color::from_u32(0x282a36),
            accent: Color::from_u32(0xbd93f9),
            dim: Color::from_u32(0x6272a4),
            highlight_bg: Color::from_u32(0x44475a),
            warn: Color::from_u32(0xff5555),
            border: Color::from_u32(0x44475a),
            syntax_keyword: Color::from_u32(0xff79c6),
        }
    }

    pub fn nord() -> Self {
        // https://www.nordtheme.com/docs/colors-and-palettes
        Self {
            fg: Color::from_u32(0xd8dee9),
            bg: Color::from_u32(0x2e3440),
            accent: Color::from_u32(0x88c0d0),
            dim: Color::from_u32(0x4c566a),
            highlight_bg: Color::from_u32(0x3b4252),
            warn: Color::from_u32(0xbf616a),
            border: Color::from_u32(0x434c5e),
            syntax_keyword: Color::from_u32(0x81a1c1),
        }
    }

    pub fn ayu_dark() -> Self {
        // https://github.com/ayu-theme/ayu-colors / v2 palette
        Self {
            fg: Color::from_u32(0xbfbdb6),
            bg: Color::from_u32(0x0b0e14),
            accent: Color::from_u32(0xffcc66),
            dim: Color::from_u32(0x565b66),
            highlight_bg: Color::from_u32(0x11151c),
            warn: Color::from_u32(0xf26d78),
            border: Color::from_u32(0x171b24),
            syntax_keyword: Color::from_u32(0xff8f40),
        }
    }

    pub fn vercel_dark() -> Self {
        // https://github.com/sst/opencode/blob/dev/packages/ui/src/theme/themes/vercel.json
        Self {
            fg: Color::from_u32(0xededed),
            bg: Color::from_u32(0x000000),
            accent: Color::from_u32(0x0070f3),
            dim: Color::from_u32(0x878787),
            highlight_bg: Color::from_u32(0x111111),
            warn: Color::from_u32(0xe5484d),
            border: Color::from_u32(0x2a2a2a),
            syntax_keyword: Color::from_u32(0xf75590),
        }
    }

    pub fn github_light() -> Self {
        Self {
            fg: Color::from_u32(0x24292f),
            bg: Color::from_u32(0xffffff),
            accent: Color::from_u32(0x0969da),
            dim: Color::from_u32(0x57606a),
            highlight_bg: Color::from_u32(0xddf4ff),
            warn: Color::from_u32(0xcf222e),
            border: Color::from_u32(0xd0d7de),
            syntax_keyword: Color::from_u32(0xcf222e),
        }
    }
}

/// Builtin themes (name -> constructor). Cheap to enumerate.
pub static BUILTINS: &[(&str, Theme)] = &[
    (
        "opencode",
        Theme {
            fg: Color::from_u32(0xeeeeee),
            bg: Color::from_u32(0x0a0a0a),
            accent: Color::from_u32(0xfab283),
            dim: Color::from_u32(0x808080),
            highlight_bg: Color::from_u32(0x282828),
            warn: Color::from_u32(0xe06c75),
            border: Color::from_u32(0x3c3c3c),
            syntax_keyword: Color::from_u32(0x5c9cf5),
        },
    ),
    (
        "monokai",
        Theme {
            fg: Color::from_u32(0xf8f8f2),
            bg: Color::from_u32(0x272822),
            accent: Color::from_u32(0xae81ff),
            dim: Color::from_u32(0x75715e),
            highlight_bg: Color::from_u32(0x3e3d32),
            warn: Color::from_u32(0xf92672),
            border: Color::from_u32(0x49483e),
            syntax_keyword: Color::from_u32(0xf92672),
        },
    ),
    (
        "github-dark",
        Theme {
            fg: Color::from_u32(0xc9d1d9),
            bg: Color::from_u32(0x0d1117),
            accent: Color::from_u32(0x58a6ff),
            dim: Color::from_u32(0x8b949e),
            highlight_bg: Color::from_u32(0x1f6feb),
            warn: Color::from_u32(0xf85149),
            border: Color::from_u32(0x30363d),
            syntax_keyword: Color::from_u32(0xff7b72),
        },
    ),
    (
        "github-light",
        Theme {
            fg: Color::from_u32(0x24292f),
            bg: Color::from_u32(0xffffff),
            accent: Color::from_u32(0x0969da),
            dim: Color::from_u32(0x57606a),
            highlight_bg: Color::from_u32(0xddf4ff),
            warn: Color::from_u32(0xcf222e),
            border: Color::from_u32(0xd0d7de),
            syntax_keyword: Color::from_u32(0xcf222e),
        },
    ),
    (
        "dracula",
        Theme {
            fg: Color::from_u32(0xf8f8f2),
            bg: Color::from_u32(0x282a36),
            accent: Color::from_u32(0xbd93f9),
            dim: Color::from_u32(0x6272a4),
            highlight_bg: Color::from_u32(0x44475a),
            warn: Color::from_u32(0xff5555),
            border: Color::from_u32(0x44475a),
            syntax_keyword: Color::from_u32(0xff79c6),
        },
    ),
    (
        "nord",
        Theme {
            fg: Color::from_u32(0xd8dee9),
            bg: Color::from_u32(0x2e3440),
            accent: Color::from_u32(0x88c0d0),
            dim: Color::from_u32(0x4c566a),
            highlight_bg: Color::from_u32(0x3b4252),
            warn: Color::from_u32(0xbf616a),
            border: Color::from_u32(0x434c5e),
            syntax_keyword: Color::from_u32(0x81a1c1),
        },
    ),
    (
        "ayu-dark",
        Theme {
            fg: Color::from_u32(0xbfbdb6),
            bg: Color::from_u32(0x0b0e14),
            accent: Color::from_u32(0xffcc66),
            dim: Color::from_u32(0x565b66),
            highlight_bg: Color::from_u32(0x11151c),
            warn: Color::from_u32(0xf26d78),
            border: Color::from_u32(0x171b24),
            syntax_keyword: Color::from_u32(0xff8f40),
        },
    ),
    (
        "vercel-dark",
        Theme {
            fg: Color::from_u32(0xededed),
            bg: Color::from_u32(0x000000),
            accent: Color::from_u32(0x0070f3),
            dim: Color::from_u32(0x878787),
            highlight_bg: Color::from_u32(0x111111),
            warn: Color::from_u32(0xe5484d),
            border: Color::from_u32(0x2a2a2a),
            syntax_keyword: Color::from_u32(0xf75590),
        },
    ),
];

pub fn by_name(name: &str) -> Option<Theme> {
    BUILTINS.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// File schema for `.ocoger/themes/<name>.toml`.
#[derive(Debug, Deserialize)]
struct ThemeFile {
    // Accept any of: "0xRRGGBB" | "#RRGGBB" | "rgb(r,g,b)". Stored as str so
    // a single value is portable across TOML parsers.
    fg: String,
    bg: String,
    accent: String,
    dim: String,
    highlight_bg: String,
    warn: String,
    border: String,
    syntax_keyword: String,
}

/// Load every `<dir>/*.toml` under `<project>/.ocoger/themes/` as a custom theme.
/// Returns name -> Theme map. Bad entries are skipped with a warning string so
/// the UI can surface them alongside other `[theme]` warnings.
pub fn load_custom_from(dir: &std::path::Path) -> (HashMap<String, Theme>, Vec<String>) {
    let mut out = HashMap::new();
    let mut warnings = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return (out, warnings),
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("theme")
            .to_string();
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                warnings.push(format!("{}: read: {e}", name));
                continue;
            }
        };
        let parsed: ThemeFile = match toml::from_str(&raw) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(format!("{}: {e}", name));
                continue;
            }
        };
        match file_to_theme(&parsed) {
            Ok(t) => {
                out.insert(name, t);
            }
            Err(e) => warnings.push(format!("{}: {e}", name)),
        }
    }
    (out, warnings)
}

fn file_to_theme(f: &ThemeFile) -> Result<Theme, String> {
    Ok(Theme {
        fg: parse_color(&f.fg)?,
        bg: parse_color(&f.bg)?,
        accent: parse_color(&f.accent)?,
        dim: parse_color(&f.dim)?,
        highlight_bg: parse_color(&f.highlight_bg)?,
        warn: parse_color(&f.warn)?,
        border: parse_color(&f.border)?,
        syntax_keyword: parse_color(&f.syntax_keyword)?,
    })
}

/// "0xRRGGBB" | "#RRGGBB" | "rgb(r,g,b)". Whitespace tolerated.
fn parse_color(s: &str) -> Result<Color, String> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix('#')) {
        let v = u32::from_str_radix(hex, 16).map_err(|e| format!("bad hex '{t}': {e}"))?;
        if v > 0xFFFFFF {
            return Err(format!("hex out of range: {t}"));
        }
        return Ok(Color::from_u32(v));
    }
    if t.starts_with("rgb(") && t.ends_with(')') {
        let inner = &t[4..t.len() - 1];
        let parts: Vec<_> = inner.split(',').map(|p| p.trim().parse::<u8>()).collect();
        if parts.len() != 3 || parts.iter().any(|p| p.is_err()) {
            return Err(format!("bad rgb '{t}'"));
        }
        let r = parts[0].clone().unwrap();
        let g = parts[1].clone().unwrap();
        let b = parts[2].clone().unwrap();
        return Ok(Color::Rgb(r, g, b));
    }
    Err(format!(
        "unsupported color '{t}' (expect 0xRRGGBB / #RRGGBB / rgb(r,g,b))"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn builtins_lookup_by_name() {
        assert_eq!(by_name("monokai").unwrap().warn, Color::from_u32(0xf92672));
        assert_eq!(
            by_name("github-light").unwrap().bg,
            Color::from_u32(0xffffff)
        );
        assert_eq!(by_name("opencode"), Some(Theme::default()));
        assert!(by_name("nope").is_none());
    }

    #[test]
    fn parse_color_handles_hex_and_rgb_and_errors() {
        assert_eq!(parse_color("0xff0000").unwrap(), Color::from_u32(0xff0000));
        assert_eq!(parse_color("#e6db74").unwrap(), Color::from_u32(0xe6db74));
        assert_eq!(
            parse_color("rgb(255, 0, 0)").unwrap(),
            Color::Rgb(255, 0, 0)
        );
        assert!(parse_color("no").is_err());
        assert!(parse_color("#gg0000").is_err());
    }

    #[test]
    fn load_custom_theme_from_disk() {
        let dir = std::env::temp_dir().join(format!(
            "ocoger-theme-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("mytheme.toml"),
            r#"
fg = "0xeeeeee"
bg = "0x0a0a0a"
accent = "0xfab283"
dim = "0x808080"
highlight_bg = "0x282828"
warn = "0xe06c75"
border = "0x3c3c3c"
syntax_keyword = "0x5c9cf5"
"#,
        )
        .unwrap();
        fs::write(dir.join("broken.toml"), "fg = 12345\nbg = \"nope\"\n").unwrap();
        let (map, warnings) = load_custom_from(&dir);
        assert!(map.contains_key("mytheme"));
        assert!(!map.contains_key("broken"), "bad entries skipped");
        assert!(warnings.iter().any(|w| w.contains("broken")));
        let got = map.get("mytheme").unwrap();
        assert_eq!(got.fg, Color::from_u32(0xeeeeee));
        assert_eq!(got.accent, Color::from_u32(0xfab283));
    }
}
