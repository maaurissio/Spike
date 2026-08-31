use std::{env, fs, path::PathBuf};

use crate::config::Theme;
use ratatui::style::{Color, Modifier, Style};

const PALETTE_TEMPLATE: &str = r##"# Paleta editable de Spike. Guarda el archivo y pulsa F5 en la app.
# Formato: #RRGGBB. `title` controla encabezados; `body`, el contenido normal.
[dark]
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
    let mut contents = fs::read_to_string(&path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    // Las primeras versiones solo incluían una paleta plana para Noche.
    // Hacer explícita su sección conserva los colores y evita confundirlos
    // con las claves equivalentes del tema Claro.
    let mut migrated = false;
    if !contents.lines().any(|line| line.trim() == "[dark]") {
        contents = format!("[dark]\n{contents}");
        migrated = true;
    }
    // Añadir las claves nuevas conserva cualquier color oscuro ya editado.
    if !contents.lines().any(|line| line.trim() == "[light]") {
        contents.push_str(
            r##"

# Añadido automáticamente: títulos separados y tema Claro editable.
title = "#8ec07c"

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
"##,
        );
        migrated = true;
    }
    if migrated {
        fs::write(&path, &contents)
            .map_err(|error| format!("no se pudo actualizar {}: {error}", path.display()))?;
    }
    parse_palette(&contents).map_err(|error| format!("{}: {error}", path.display()))
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
            value
                if (value.starts_with("ascendente") || value.starts_with("asc")) && light_theme =>
            {
                Color::Rgb(121, 116, 14)
            }
            value if value.starts_with("ascendente") || value.starts_with("asc") => {
                Color::Rgb(184, 187, 38)
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

    /// Cada premade conserva un color estable. La tabla usa este color en el
    /// punto previo al nombre y el detalle mantiene la etiqueta textual.
    pub fn premade_style(&self, label: &str) -> Style {
        if self.theme == Theme::Mono {
            return self.base;
        }
        match label.strip_prefix("Grupo ").unwrap_or(label) {
            "A" => self.focus,
            "B" => self.pending,
            "C" => self.rank,
            "D" => self.good,
            _ => self.dim,
        }
        .add_modifier(Modifier::BOLD)
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
            Some(Color::Rgb(184, 187, 38))
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
        assert!(parse_palette("unknown = \"#000000\"").is_err());
        assert!(parse_palette("text = \"#xyzxyz\"").is_err());
    }
}
