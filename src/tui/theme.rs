use crate::config::Theme;
use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    theme: Theme,
    pub base: Style,
    pub dim: Style,
    pub border: Style,
    pub focus: Style,
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
}
