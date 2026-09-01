use std::{env, fs, path::PathBuf};

use crate::config::Theme;
use ratatui::style::{Color, Modifier, Style};

const PALETTE_TEMPLATE: &str = r##"[dark]
background = "#282828"
body = "#ebdbb2"
title = "#8ec07c"
muted = "#928374"
border = "#7c6f64"
primary = "#8ec07c"
win = "#b8bb26"
loss = "#fb4934"
rank = "#d3869b"
warning = "#fabd2f"
selection = "#504945"
logo_primary = "#fabd2f"
logo_secondary = "#ebdbb2"
logo_fade = "#928374"
cpu = "#b8bb26"
ram = "#d3869b"
chart_win = "#8ec07c"
chart_loss = "#fb4934"
log_info = "#83a598"
log_success = "#b8bb26"
log_warning = "#fe8019"

[light]
background = "#fbf1c7"
body = "#3c3836"
title = "#427b58"
muted = "#7c6f64"
border = "#a89984"
primary = "#427b58"
win = "#79740e"
loss = "#9d0006"
rank = "#8f3f71"
warning = "#b57614"
selection = "#d5c4a1"
logo_primary = "#b57614"
logo_secondary = "#3c3836"
logo_fade = "#7c6f64"
cpu = "#79740e"
ram = "#8f3f71"
chart_win = "#427b58"
chart_loss = "#9d0006"
log_info = "#076678"
log_success = "#79740e"
log_warning = "#af3a03"
"##;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PaletteColors {
    background: Color,
    body: Color,
    title: Color,
    muted: Color,
    border: Color,
    primary: Color,
    win: Color,
    loss: Color,
    rank: Color,
    warning: Color,
    selection: Color,
    logo_primary: Color,
    logo_secondary: Color,
    logo_fade: Color,
    cpu: Color,
    ram: Color,
    chart_win: Color,
    chart_loss: Color,
    log_info: Color,
    log_success: Color,
    log_warning: Color,
}

impl PaletteColors {
    fn dark() -> Self {
        Self {
            background: Color::Rgb(40, 40, 40),
            body: Color::Rgb(235, 219, 178),
            title: Color::Rgb(142, 192, 124),
            muted: Color::Rgb(146, 131, 116),
            border: Color::Rgb(124, 111, 100),
            primary: Color::Rgb(142, 192, 124),
            win: Color::Rgb(184, 187, 38),
            loss: Color::Rgb(251, 73, 52),
            rank: Color::Rgb(211, 134, 155),
            warning: Color::Rgb(250, 189, 47),
            selection: Color::Rgb(80, 73, 69),
            logo_primary: Color::Rgb(250, 189, 47),
            logo_secondary: Color::Rgb(235, 219, 178),
            logo_fade: Color::Rgb(146, 131, 116),
            cpu: Color::Rgb(184, 187, 38),
            ram: Color::Rgb(211, 134, 155),
            chart_win: Color::Rgb(142, 192, 124),
            chart_loss: Color::Rgb(251, 73, 52),
            log_info: Color::Rgb(131, 165, 152),
            log_success: Color::Rgb(184, 187, 38),
            log_warning: Color::Rgb(254, 128, 25),
        }
    }

    fn light() -> Self {
        Self {
            background: Color::Rgb(251, 241, 199),
            body: Color::Rgb(60, 56, 54),
            title: Color::Rgb(66, 123, 88),
            muted: Color::Rgb(124, 111, 100),
            border: Color::Rgb(168, 153, 132),
            primary: Color::Rgb(66, 123, 88),
            win: Color::Rgb(121, 116, 14),
            loss: Color::Rgb(157, 0, 6),
            rank: Color::Rgb(143, 63, 113),
            warning: Color::Rgb(181, 118, 20),
            selection: Color::Rgb(213, 196, 161),
            logo_primary: Color::Rgb(181, 118, 20),
            logo_secondary: Color::Rgb(60, 56, 54),
            logo_fade: Color::Rgb(124, 111, 100),
            cpu: Color::Rgb(121, 116, 14),
            ram: Color::Rgb(143, 63, 113),
            chart_win: Color::Rgb(66, 123, 88),
            chart_loss: Color::Rgb(157, 0, 6),
            log_info: Color::Rgb(7, 102, 120),
            log_success: Color::Rgb(121, 116, 14),
            log_warning: Color::Rgb(175, 58, 3),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EditablePalette {
    dark: PaletteColors,
    light: PaletteColors,
}

impl Default for EditablePalette {
    fn default() -> Self {
        Self {
            dark: PaletteColors::dark(),
            light: PaletteColors::light(),
        }
    }
}

pub(super) fn palette_path() -> Option<PathBuf> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("spike").join("palette.toml"))
}

pub(super) fn load_or_create_palette() -> Result<EditablePalette, String> {
    let path = palette_path().ok_or_else(|| "APPDATA no está disponible".to_string())?;
    if !path.exists() {
        let parent = path
            .parent()
            .ok_or_else(|| "ruta de paleta inválida".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::write(&path, PALETTE_TEMPLATE).map_err(|error| error.to_string())?;
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let palette =
        parse_palette(&contents).map_err(|error| format!("{}: {error}", path.display()))?;
    let normalized = encode_palette(&palette);
    if contents != normalized {
        fs::write(&path, &normalized)
            .map_err(|error| format!("no se pudo actualizar {}: {error}", path.display()))?;
    }
    Ok(palette)
}

fn encode_palette(palette: &EditablePalette) -> String {
    format!(
        "[dark]\n{}\n[light]\n{}",
        encode_colors(&palette.dark),
        encode_colors(&palette.light)
    )
}

fn encode_colors(colors: &PaletteColors) -> String {
    format!(
        "background = \"{}\"\nbody = \"{}\"\ntitle = \"{}\"\nmuted = \"{}\"\nborder = \"{}\"\nprimary = \"{}\"\nwin = \"{}\"\nloss = \"{}\"\nrank = \"{}\"\nwarning = \"{}\"\nselection = \"{}\"\nlogo_primary = \"{}\"\nlogo_secondary = \"{}\"\nlogo_fade = \"{}\"\ncpu = \"{}\"\nram = \"{}\"\nchart_win = \"{}\"\nchart_loss = \"{}\"\nlog_info = \"{}\"\nlog_success = \"{}\"\nlog_warning = \"{}\"\n",
        color_hex(colors.background),
        color_hex(colors.body),
        color_hex(colors.title),
        color_hex(colors.muted),
        color_hex(colors.border),
        color_hex(colors.primary),
        color_hex(colors.win),
        color_hex(colors.loss),
        color_hex(colors.rank),
        color_hex(colors.warning),
        color_hex(colors.selection),
        color_hex(colors.logo_primary),
        color_hex(colors.logo_secondary),
        color_hex(colors.logo_fade),
        color_hex(colors.cpu),
        color_hex(colors.ram),
        color_hex(colors.chart_win),
        color_hex(colors.chart_loss),
        color_hex(colors.log_info),
        color_hex(colors.log_success),
        color_hex(colors.log_warning),
    )
}

fn color_hex(color: Color) -> String {
    let Color::Rgb(red, green, blue) = color else {
        unreachable!("las paletas editables solo almacenan RGB")
    };
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn parse_palette(contents: &str) -> Result<EditablePalette, String> {
    let mut palette = EditablePalette::default();
    let mut section = Theme::Dark;
    for (index, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            section = match line {
                "[dark]" => Theme::Dark,
                "[light]" => Theme::Light,
                _ => return Err(format!("línea {}: sección desconocida `{line}`", index + 1)),
            };
            continue;
        }
        let (key, raw_value) = line
            .split_once('=')
            .ok_or_else(|| format!("línea {}: se esperaba clave = \"#RRGGBB\"", index + 1))?;
        let color = parse_hex(raw_value.trim())
            .map_err(|error| format!("línea {}: {} ({error})", index + 1, key.trim()))?;
        let colors = if section == Theme::Light {
            &mut palette.light
        } else {
            &mut palette.dark
        };
        match key.trim() {
            "background" => colors.background = color,
            "body" | "text" => colors.body = color,
            "title" => colors.title = color,
            "muted" => colors.muted = color,
            "border" => colors.border = color,
            "primary" => colors.primary = color,
            "win" => colors.win = color,
            "loss" => colors.loss = color,
            "rank" => colors.rank = color,
            "warning" => colors.warning = color,
            "selection" => colors.selection = color,
            "logo_primary" => colors.logo_primary = color,
            "logo_secondary" => colors.logo_secondary = color,
            "logo_fade" => colors.logo_fade = color,
            "cpu" => colors.cpu = color,
            "ram" => colors.ram = color,
            "chart_win" => colors.chart_win = color,
            "chart_loss" => colors.chart_loss = color,
            "log_info" => colors.log_info = color,
            "log_success" => colors.log_success = color,
            "log_warning" => colors.log_warning = color,
            unknown => {
                return Err(format!(
                    "línea {}: color desconocido `{unknown}`",
                    index + 1
                ));
            }
        }
    }
    Ok(palette)
}

fn parse_hex(value: &str) -> Result<Color, &'static str> {
    let value = value.trim_matches('"');
    let hex = value.strip_prefix('#').ok_or("falta el prefijo #")?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("usa exactamente seis dígitos hexadecimales");
    }
    let component = |range| u8::from_str_radix(&hex[range], 16).expect("hexadecimal validado");
    Ok(Color::Rgb(
        component(0..2),
        component(2..4),
        component(4..6),
    ))
}

#[derive(Clone, Copy)]
pub(super) struct Palette {
    theme: Theme,
    pub base: Style,
    pub dim: Style,
    pub border: Style,
    pub focus: Style,
    pub title: Style,
    pub selected: Style,
    pub good: Style,
    pub bad: Style,
    pub rank: Style,
    pub pending: Style,
    pub logo_primary: Style,
    pub logo_secondary: Style,
    pub logo_fade: Style,
    pub cpu: Style,
    pub ram: Style,
    pub chart_win: Style,
    pub chart_loss: Style,
    pub log_info: Style,
    pub log_success: Style,
    pub log_warning: Style,
}

impl Palette {
    pub fn new(theme: Theme) -> Self {
        let (bg, fg, dim, border, cyan, green, red, purple, amber, selection) = match theme {
            // Gruvbox Dark (contraste medio). Se usan los valores RGB
            // originales para no depender de la tabla de 16 colores del host.
            Theme::Dark => (
                Color::Rgb(40, 40, 40),    // dark0
                Color::Rgb(235, 219, 178), // light1
                Color::Rgb(146, 131, 116), // gray
                Color::Rgb(124, 111, 100), // dark4
                Color::Rgb(142, 192, 124), // bright aqua
                Color::Rgb(184, 187, 38),  // bright green
                Color::Rgb(251, 73, 52),   // bright red
                Color::Rgb(211, 134, 155), // bright purple
                Color::Rgb(250, 189, 47),  // bright yellow
                Color::Rgb(80, 73, 69),    // dark2
            ),
            // Gruvbox Light. Las variantes faded conservan contraste sobre
            // light0 sin alterar el significado de los colores semánticos.
            Theme::Light => (
                Color::Rgb(251, 241, 199), // light0
                Color::Rgb(60, 56, 54),    // dark1
                Color::Rgb(124, 111, 100), // dark4
                Color::Rgb(168, 153, 132), // light4
                Color::Rgb(66, 123, 88),   // faded aqua
                Color::Rgb(121, 116, 14),  // faded green
                Color::Rgb(157, 0, 6),     // faded red
                Color::Rgb(143, 63, 113),  // faded purple
                Color::Rgb(181, 118, 20),  // faded yellow
                Color::Rgb(213, 196, 161), // light2
            ),
            // Sistema conserva fondo/texto del terminal, pero usa una paleta
            // semántica brillante y estable cercana al prototipo.
            Theme::System => (
                Color::Reset,
                Color::Reset,
                Color::DarkGray,
                Color::DarkGray,
                Color::LightCyan,
                Color::LightGreen,
                Color::LightRed,
                Color::LightMagenta,
                Color::LightYellow,
                Color::Reset,
            ),
            Theme::Mono => (
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
            ),
        };
        let base = Style::default().fg(fg).bg(bg);
        if theme == Theme::Mono {
            return Self {
                theme,
                base,
                dim: base,
                border: base,
                focus: base.add_modifier(Modifier::BOLD),
                title: base.add_modifier(Modifier::BOLD),
                selected: base.add_modifier(Modifier::REVERSED),
                good: base,
                bad: base,
                rank: base,
                pending: base,
                logo_primary: base.add_modifier(Modifier::BOLD),
                logo_secondary: base.add_modifier(Modifier::BOLD),
                logo_fade: base,
                cpu: base,
                ram: base,
                chart_win: base,
                chart_loss: base,
                log_info: base,
                log_success: base,
                log_warning: base,
            };
        }
        Self {
            theme,
            base,
            dim: base.fg(dim),
            border: base.fg(border),
            focus: base.fg(cyan).add_modifier(Modifier::BOLD),
            title: base.fg(cyan).add_modifier(Modifier::BOLD),
            selected: if theme == Theme::System {
                base.add_modifier(Modifier::REVERSED)
            } else {
                base.bg(selection)
            },
            good: base.fg(green),
            bad: base.fg(red),
            rank: base.fg(purple),
            pending: base.fg(amber),
            logo_primary: base.fg(amber).add_modifier(Modifier::BOLD),
            logo_secondary: base.add_modifier(Modifier::BOLD),
            logo_fade: base.fg(dim),
            cpu: base.fg(green),
            ram: base.fg(purple),
            chart_win: base.fg(cyan),
            chart_loss: base.fg(red),
            log_info: base.fg(cyan),
            log_success: base.fg(green),
            log_warning: base.fg(amber),
        }
    }

    pub fn with_custom(theme: Theme, custom: Option<&EditablePalette>) -> Self {
        let Some(custom) = custom.filter(|_| matches!(theme, Theme::Dark | Theme::Light)) else {
            return Self::new(theme);
        };
        let colors = if theme == Theme::Light {
            &custom.light
        } else {
            &custom.dark
        };
        let base = Style::default().fg(colors.body).bg(colors.background);
        Self {
            theme,
            base,
            dim: base.fg(colors.muted),
            border: base.fg(colors.border),
            focus: base.fg(colors.primary).add_modifier(Modifier::BOLD),
            title: base.fg(colors.title).add_modifier(Modifier::BOLD),
            selected: base.bg(colors.selection),
            good: base.fg(colors.win),
            bad: base.fg(colors.loss),
            rank: base.fg(colors.rank),
            pending: base.fg(colors.warning),
            logo_primary: base.fg(colors.logo_primary).add_modifier(Modifier::BOLD),
            logo_secondary: base.fg(colors.logo_secondary).add_modifier(Modifier::BOLD),
            logo_fade: base.fg(colors.logo_fade),
            cpu: base.fg(colors.cpu),
            ram: base.fg(colors.ram),
            chart_win: base.fg(colors.chart_win),
            chart_loss: base.fg(colors.chart_loss),
            log_info: base.fg(colors.log_info),
            log_success: base.fg(colors.log_success),
            log_warning: base.fg(colors.log_warning),
        }
    }

    /// Color por familia competitiva. Los tonos del tema oscuro corresponden
    /// al catálogo visual vigente; Claro usa variantes más oscuras para
    /// conservar contraste sobre fondo blanco. El rango nunca depende solo del
    /// color: su nombre permanece visible.
    pub fn rank_style(&self, label: &str) -> Style {
        if self.theme == Theme::Mono {
            return self.base;
        }
        let label = label.to_lowercase();
        let family = label.split_whitespace().next().unwrap_or_default();
        let light_theme = self.theme == Theme::Light;
        let color = match family {
            value if value.starts_with("hierro") || value.starts_with("hie") => {
                Color::Rgb(146, 131, 116)
            }
            value if (value.starts_with("bronce") || value.starts_with("bro")) && light_theme => {
                Color::Rgb(175, 58, 3)
            }
            value if value.starts_with("bronce") || value.starts_with("bro") => {
                Color::Rgb(254, 128, 25)
            }
            value
                if (value == "plata" || (value.starts_with("pla") && value != "platino"))
                    && light_theme =>
            {
                Color::Rgb(102, 92, 84)
            }
            value if value == "plata" || (value.starts_with("pla") && value != "platino") => {
                Color::Rgb(213, 196, 161)
            }
            value if value.starts_with("oro") && light_theme => Color::Rgb(181, 118, 20),
            value if value.starts_with("oro") => Color::Rgb(250, 189, 47),
            value if (value.starts_with("platino") || value.starts_with("plt")) && light_theme => {
                Color::Rgb(66, 123, 88)
            }
            value if value.starts_with("platino") || value.starts_with("plt") => {
                Color::Rgb(142, 192, 124)
            }
            value if (value.starts_with("diamante") || value.starts_with("dia")) && light_theme => {
                Color::Rgb(7, 102, 120)
            }
            value if value.starts_with("diamante") || value.starts_with("dia") => {
                Color::Rgb(131, 165, 152)
            }
            value if value.starts_with("ascendente") || value.starts_with("asc") => {
                Color::Rgb(142, 192, 124)
            }
            value if (value.starts_with("inmortal") || value.starts_with("inm")) && light_theme => {
                Color::Rgb(143, 63, 113)
            }
            value if value.starts_with("inmortal") || value.starts_with("inm") => {
                Color::Rgb(211, 134, 155)
            }
            value if (value.starts_with("radiante") || value.starts_with("rad")) && light_theme => {
                Color::Rgb(181, 118, 20)
            }
            value if value.starts_with("radiante") || value.starts_with("rad") => {
                Color::Rgb(250, 189, 47)
            }
            _ => return self.rank,
        };
        self.base.fg(color).add_modifier(Modifier::BOLD)
    }

    /// Cada premade conserva un color pastel estable en su punto. Las
    /// etiquetas Grupo A/B son internas y nunca se muestran al jugador.
    pub fn premade_style(&self, label: &str) -> Style {
        let index = match label.strip_prefix("Grupo ").unwrap_or(label) {
            "A" => 1,
            "B" => 2,
            "C" => 3,
            "D" => 4,
            "E" => 5,
            _ => 0,
        };
        self.premade_index_style(index)
    }

    pub fn premade_index_style(&self, index: u8) -> Style {
        if self.theme == Theme::Mono {
            return self.base;
        }
        let light = self.theme == Theme::Light;
        let color = match index {
            1 if light => Color::Rgb(91, 127, 189),
            1 => Color::Rgb(137, 180, 250),
            2 if light => Color::Rgb(179, 92, 154),
            2 => Color::Rgb(245, 194, 231),
            3 if light => Color::Rgb(95, 143, 104),
            3 => Color::Rgb(166, 227, 161),
            4 if light => Color::Rgb(168, 120, 50),
            4 => Color::Rgb(249, 226, 175),
            5 if light => Color::Rgb(128, 101, 168),
            5 => Color::Rgb(203, 166, 247),
            _ => return self.dim,
        };
        self.base.fg(color).add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_families_use_distinct_catalog_colors() {
        let palette = Palette::new(Theme::Dark);

        assert_eq!(
            palette.rank_style("Ascendente 2").fg,
            Some(Color::Rgb(142, 192, 124))
        );
        assert_eq!(
            palette.rank_style("Diamante 2").fg,
            Some(Color::Rgb(131, 165, 152))
        );
        assert_ne!(
            palette.rank_style("Ascendente 2").fg,
            palette.rank_style("Diamante 2").fg
        );
        assert_eq!(
            palette.rank_style("ASC2").fg,
            palette.rank_style("Ascendente 2").fg
        );
        assert_eq!(
            Palette::new(Theme::Light).rank_style("Ascendente 2").fg,
            Some(Color::Rgb(142, 192, 124))
        );
    }

    #[test]
    fn dark_and_light_use_the_official_gruvbox_base_colors() {
        let dark = Palette::new(Theme::Dark);
        assert_eq!(dark.base.bg, Some(Color::Rgb(40, 40, 40)));
        assert_eq!(dark.base.fg, Some(Color::Rgb(235, 219, 178)));
        assert_eq!(dark.good.fg, Some(Color::Rgb(184, 187, 38)));

        let light = Palette::new(Theme::Light);
        assert_eq!(light.base.bg, Some(Color::Rgb(251, 241, 199)));
        assert_eq!(light.base.fg, Some(Color::Rgb(60, 56, 54)));
        assert_eq!(light.bad.fg, Some(Color::Rgb(157, 0, 6)));
    }

    #[test]
    fn premade_groups_keep_distinct_semantic_colors() {
        let palette = Palette::new(Theme::Dark);

        assert_ne!(
            palette.premade_style("Grupo A").fg,
            palette.premade_style("Grupo B").fg
        );
        assert_eq!(palette.premade_style("Solo").fg, palette.dim.fg);
    }

    #[test]
    fn editable_palette_parses_hex_colors_and_rejects_unknown_keys() {
        let colors = parse_palette(
            "[dark]\nbackground = \"#010203\"\ntitle = \"#445566\"\nprimary = \"#aabbcc\"\nwin = \"#102030\"\n[light]\nbody = \"#112233\"",
        )
        .unwrap();
        let palette = Palette::with_custom(Theme::Dark, Some(&colors));
        assert_eq!(palette.base.bg, Some(Color::Rgb(1, 2, 3)));
        assert_eq!(palette.title.fg, Some(Color::Rgb(68, 85, 102)));
        assert_eq!(palette.focus.fg, Some(Color::Rgb(170, 187, 204)));
        assert_eq!(palette.good.fg, Some(Color::Rgb(16, 32, 48)));
        let light = Palette::with_custom(Theme::Light, Some(&colors));
        assert_eq!(light.base.fg, Some(Color::Rgb(17, 34, 51)));
        let normalized = encode_palette(&colors);
        assert!(normalized.starts_with("[dark]\nbackground"));
        assert!(normalized.contains("\nbody = \"#ebdbb2\"\ntitle = \"#445566\""));
        assert!(normalized.contains("\n[light]\nbackground"));
        assert!(!normalized.contains("text ="));
        assert!(!normalized.lines().any(|line| line.starts_with('#')));
        assert!(parse_palette("unknown = \"#000000\"").is_err());
        assert!(parse_palette("text = \"#xyzxyz\"").is_err());
    }
}
