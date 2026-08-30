use crate::config::Theme;
use ratatui::style::{Color, Modifier, Style};

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
            Theme::Dark => (
                Color::Rgb(15, 22, 33),
                Color::Rgb(220, 230, 239),
                Color::Rgb(150, 168, 188),
                Color::Rgb(80, 98, 122),
                Color::Rgb(114, 216, 242),
                Color::Rgb(132, 215, 164),
                Color::Rgb(255, 145, 167),
                Color::Rgb(198, 165, 245),
                Color::Rgb(236, 198, 129),
                Color::Rgb(32, 53, 75),
            ),
            Theme::Light => (
                Color::Rgb(245, 247, 250),
                Color::Rgb(32, 52, 73),
                Color::Rgb(82, 101, 121),
                Color::Rgb(128, 145, 165),
                Color::Rgb(0, 101, 120),
                Color::Rgb(21, 104, 72),
                Color::Rgb(163, 47, 74),
                Color::Rgb(112, 67, 161),
                Color::Rgb(130, 87, 8),
                Color::Rgb(219, 234, 242),
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
        let dark = self.theme == Theme::Light;
        let color = match family {
            "hierro" => Color::Rgb(134, 137, 134),
            "bronce" if dark => Color::Rgb(124, 85, 34),
            "bronce" => Color::Rgb(165, 133, 93),
            "plata" if dark => Color::Rgb(89, 101, 105),
            "plata" => Color::Rgb(187, 194, 194),
            "oro" if dark => Color::Rgb(122, 97, 0),
            "oro" => Color::Rgb(236, 207, 86),
            "platino" if dark => Color::Rgb(25, 111, 120),
            "platino" => Color::Rgb(89, 169, 182),
            "diamante" if dark => Color::Rgb(112, 67, 161),
            "diamante" => Color::Rgb(180, 137, 196),
            "ascendente" if dark => Color::Rgb(28, 114, 69),
            "ascendente" => Color::Rgb(106, 226, 175),
            "inmortal" if dark => Color::Rgb(148, 40, 72),
            "inmortal" => Color::Rgb(187, 61, 101),
            "radiante" if dark => Color::Rgb(138, 106, 0),
            "radiante" => Color::Rgb(255, 255, 170),
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
            Some(Color::Rgb(106, 226, 175))
        );
        assert_eq!(
            palette.rank_style("Diamante 2").fg,
            Some(Color::Rgb(180, 137, 196))
        );
        assert_ne!(
            palette.rank_style("Ascendente 2").fg,
            palette.rank_style("Diamante 2").fg
        );
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
