use crate::config::Theme;
use ratatui::style::{Color, Modifier, Style};

pub(super) struct Palette {
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
            // Sistema hereda fondo/texto y usa los colores ANSI del terminal.
            Theme::System | Theme::Mono => (
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Reset,
                Color::Cyan,
                Color::Green,
                Color::Red,
                Color::Magenta,
                Color::Yellow,
                Color::Reset,
            ),
        };
        let base = Style::default().fg(fg).bg(bg);
        if theme == Theme::Mono {
            return Self {
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
}
