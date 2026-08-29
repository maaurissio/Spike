//! Composición en celdas de terminal basada en docs/mockups; sin I/O.
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use super::{App, Focus, TABS, relative_time, theme::Palette};
use crate::{
    models::{
        MatchOutcome,
        roster::{DataAvailability, HistoricalStats, RosterPlayer, RosterSide, RosterSnapshot},
    },
    providers::capabilities::GamePhase,
};

pub(super) struct Screen {
    pub lines: Vec<Line<'static>>,
    pub anchor: Option<usize>,
    sections: Vec<usize>,
    width: usize,
    palette: Palette,
}

impl Screen {
    fn new(width: u16, palette: Palette) -> Self {
        Self {
            lines: vec![],
            anchor: None,
            sections: vec![],
            width: usize::from(width.max(1)),
            palette,
        }
    }

    // Las filas ya quedan partidas en celdas: scroll y selección comparten coordenadas.
    fn row(&mut self, line: impl Into<Line<'static>>) {
        let line = line.into();
        let mut spans = vec![];
        let mut used = 0;
        for span in &line.spans {
            for glyph in span.styled_graphemes(line.style) {
                let width = Span::raw(glyph.symbol).width();
                if width > self.width {
                    continue;
                }
                if used + width > self.width {
                    self.lines.push(Line::from(std::mem::take(&mut spans)));
                    used = 0;
                }
                spans.push(Span::styled(glyph.symbol.to_owned(), glyph.style));
                used += width;
            }
        }
        self.lines.push(Line::from(spans));
    }

    fn text(&mut self, text: impl AsRef<str>) {
        for line in text.as_ref().lines() {
            self.row(Line::styled(clean(line), self.palette.base));
        }
    }

    fn section(&mut self, title: &str, style: Style) {
        self.sections.push(self.lines.len());
        let title = cell(title, self.width.saturating_sub(4))
            .trim_end()
            .to_string();
        let remaining = self.width.saturating_sub(Span::raw(&title).width() + 3);
        self.row(Line::from(vec![
            Span::styled("─ ", self.palette.border),
            Span::styled(title, style),
            Span::styled(format!(" {}", "─".repeat(remaining)), self.palette.border),
        ]));
    }

    fn selected(&mut self, line: Line<'static>, selected: bool, app: &App) {
        if selected {
            self.anchor = Some(self.lines.len());
            let style = if app.focus == Focus::Content {
                self.palette.selected
            } else {
                self.palette.base
            };
            let mut line = line;
            for span in &mut line.spans {
                span.style = span.style.patch(style);
            }
            self.row(line);
        } else {
            self.row(line);
        }
    }
}

/// Evitar que texto de un proveedor altere el terminal; nunca interpretar ANSI.
fn clean(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

fn cell(value: &str, width: usize) -> String {
    let value = clean(value);
    let span = Span::raw(value.as_str());
    let mut output = String::new();
    let mut used = 0;
    for glyph in span.styled_graphemes(Style::default()) {
        let size = Span::raw(glyph.symbol).width();
        if used + size > width {
            break;
        }
        output.push_str(glyph.symbol);
        used += size;
    }
    output.push_str(&" ".repeat(width.saturating_sub(used)));
    output
}

pub(super) fn content(app: &App, width: u16) -> Screen {
    let mut s = Screen::new(width, Palette::new(app.settings.draft.theme));
    match app.selected_tab {
        0 => panel(&mut s, app),
        1 => match_view(&mut s, app),
        2 => profile(&mut s, app),
        3 => history(&mut s, app),
        _ => settings(&mut s, app),
    }
    if app.selected_tab < 3 && app.demo.is_none() {
        if app.context_pending {
            s.text("Cargando datos… Puedes seguir navegando.");
        } else if app.context_failed {
            s.text("No se pudo actualizar. r reintenta.");
        }
    }
    if app.refresh_failed {
        s.text("Conexión interrumpida; reintentando.");
    }
    if app.selected_tab == 4 && app.log_failed {
        s.text("No se pudo escribir el registro.");
    }
    s
}

fn panel(s: &mut Screen, app: &App) {
    s.section(
        if app.demo.is_some() {
            "NORTE / PERFIL PROPIO"
        } else {
            "PERFIL PROPIO"
        },
        s.palette.focus,
    );
    profile_summary(s, app);
    s.section("SESIÓN DE HOY", s.palette.focus);
    if let Some(demo) = &app.demo {
        let kills: u32 = demo.matches.iter().map(|m| m.kills).sum();
        let deaths: u32 = demo.matches.iter().map(|m| m.deaths).sum();
        s.text(format!(
            "3 partidas · 2 V / 1 D · K/D {:.2}\n{kills} kills / {deaths} muertes",
            kills as f64 / deaths as f64
        ));
    } else {
        s.text("Resumen de sesión: —\nTodavía no integrado.");
    }
    s.section("PARTIDA", s.palette.good);
    s.text(phase_label(app));
    if app.demo.is_some() {
        s.text("Ascent / Competitivo / Sova");
    } else if let Some(context) = &app.live_match {
        s.text(format!(
            "{} / {} / {}",
            context.map,
            context.mode,
            context.agent.as_deref().unwrap_or("—")
        ));
    }
    s.row(Line::styled("[Enter] Abrir partida", s.palette.focus));
    s.section("CONEXIÓN", s.palette.focus);
    s.text(if app.demo.is_some() {
        "DEMO · datos ficticios · sin conexión"
    } else if app.state.as_ref().is_some_and(|v| v.client_found) {
        "Cliente conectado · solo lectura"
    } else {
        "Esperando al cliente"
    });
}

fn profile_summary(s: &mut Screen, app: &App) {
    if app.demo.is_some() {
        s.row(Line::styled("DIAMANTE 2 / Nivel 142", s.palette.rank));
        s.row(Line::styled("RR ██████░░░░ 64 / 100", s.palette.rank));
    } else {
        if let Some(profile) = &app.own_profile {
            s.text(format!("Nivel {} · {} XP", profile.level, profile.xp));
        } else {
            s.text("Nivel — · XP —");
        }
        if let Some(rank) = &app.competitive {
            s.row(Line::styled(
                crate::ui::competitive_tier_label(rank.tier),
                s.palette.rank,
            ));
            s.text(format!(
                "{} RR · {} victorias / {} partidas",
                rank.ranked_rating, rank.wins, rank.games
            ));
        } else {
            s.text("Rango — · RR —");
        }
    }
}

fn profile(s: &mut Screen, app: &App) {
    s.section("PERFIL PROPIO", s.palette.focus);
    profile_summary(s, app);
    s.section("RENDIMIENTO", s.palette.focus);
    if app.demo.is_some() {
        s.text("Competitivo / últimas 20 partidas\nK/D 1.18 · WR 55% · HS 26%\n11 victorias / 9 derrotas");
        s.section("POR AGENTE", s.palette.focus);
        s.text("AGENTE     PJ    K/D    WR\nSova       12    1.24   58%\nOmen        5    1.08   40%\nKilljoy     3    1.11   67%");
    } else {
        s.text("K/D — · WR — · HS —\nAgregados pendientes en esta vista.\nDisponibles por CLI: stats --limit 5");
    }
    s.section("CAMBIOS DE RR", s.palette.focus);
    if let Some(demo) = &app.demo {
        for m in &demo.matches {
            s.row(Line::styled(
                format!(
                    "{}   {}   {:+} RR",
                    m.map,
                    if m.won { "V" } else { "D" },
                    m.rr
                ),
                if m.won { s.palette.good } else { s.palette.bad },
            ));
        }
    } else if app.competitive_updates.is_empty() {
        s.text("Sin cambios disponibles.");
    } else {
        for update in &app.competitive_updates {
            s.row(Line::styled(
                format!(
                    "{:+} RR · bono {}",
                    update.rr_earned, update.performance_bonus
                ),
                if update.rr_earned >= 0 {
                    s.palette.good
                } else {
                    s.palette.bad
                },
            ));
        }
    }
}

fn match_view(s: &mut Screen, app: &App) {
    if let Some(demo) = &app.demo {
        if demo.post {
            demo_postmatch(s, app);
            return;
        }
        s.row(Line::styled(
            "ASCENT / Competitivo · 4:2 · R7*",
            s.palette.focus,
        ));
        roster(s, app, 0);
        timeline(s, app);
        roster(s, app, 5);
        if app.detail {
            let player = &demo.players[app.player_index];
            s.section(
                &format!("{} / {}", player.name, player.agent),
                s.palette.focus,
            );
            if player.hidden {
                s.text("Identidad y estadísticas ocultas.\nNo se intentará descubrirlas.");
            } else {
                s.text(format!(
                    "{} · K/D {} · WR {}\nHS {} · ADR {} · 20 partidas",
                    player.rank, player.kd, player.wr, player.hs, player.adr
                ));
            }
            s.text(if app.tracker_notice {
                if player.hidden {
                    "Tracker.gg: identidad oculta; sin enlace."
                } else {
                    "Tracker.gg: demo sin Riot ID verificado."
                }
            } else {
                "[g] Tracker.gg (no disponible en demo)"
            });
            s.anchor = Some(s.lines.len().saturating_sub(1));
        }
        return;
    }
    if let Some(completed) = &app.completed_match {
        s.section("POSTPARTIDA / RESULTADO PROPIO", s.palette.focus);
        s.text(completed);
        return;
    }
    s.section("PARTIDA", s.palette.focus);
    s.text(phase_label(app));
    let Some(context) = &app.live_match else {
        s.text(match app.state.as_ref().map(|v| v.phase) {
            Some(GamePhase::ClientClosed) => "Abre VALORANT para conectar el tracker.",
            Some(GamePhase::Lobby | GamePhase::Idle) => {
                "Cuando empiece una partida, aparecerá aquí."
            }
            Some(GamePhase::PreGame | GamePhase::AgentSelect) => {
                "Esperando el contexto de la partida."
            }
            Some(GamePhase::PostMatch) => "Esperando el resumen final.",
            _ => "Todavía no hay una partida confirmada.",
        });
        return;
    };
    s.text(format!("{} / {}", context.map, context.mode));
    let mode = context.mode.to_ascii_lowercase();
    let continuous = [
        "deathmatch",
        "team deathmatch",
        "teamdeathmatch",
        "escalation",
        "hurm",
        "ggteam",
    ]
    .contains(&mode.as_str());
    if continuous {
        s.section("TU PARTICIPACIÓN", s.palette.good);
        s.text(format!(
            "Tú / {}\nK / D / A: — / — / —",
            context.agent.as_deref().unwrap_or("—")
        ));
        s.text("Resumen al terminar; sin timeline.");
        if let Some(roster) = &context.roster {
            if roster.participants().next().is_some() {
                s.section("PARTICIPANTES", s.palette.focus);
                live_roster(s, roster, RosterSide::Participant);
            } else {
                s.section("ALIADOS", s.palette.good);
                live_roster(s, roster, RosterSide::Ally);
                s.section("ENEMIGOS", s.palette.bad);
                live_roster(s, roster, RosterSide::Enemy);
            }
        }
    } else {
        s.section("ALIADOS", s.palette.good);
        if let Some(roster) = &context.roster {
            live_roster(s, roster, RosterSide::Ally);
        } else {
            s.text(format!(
                "Tú / {}\nRoster todavía no disponible.",
                context.agent.as_deref().unwrap_or("—"),
            ));
        }
        s.section("TUS RONDAS", s.palette.focus);
        s.row(Line::styled("—K   /   —D", s.palette.pending));
        s.text("Datos en vivo aún no disponibles.");
        s.section("ENEMIGOS", s.palette.bad);
        if let Some(roster) = &context.roster {
            live_roster(s, roster, RosterSide::Enemy);
        } else {
            s.text("Roster todavía no disponible.");
        }
    }
}

fn live_roster(s: &mut Screen, roster: &RosterSnapshot, side: RosterSide) {
    let wide = s.width >= 70;
    let players = roster
        .players
        .iter()
        .filter(|player| player.side == side)
        .collect::<Vec<_>>();
    if players.is_empty() {
        s.text("Sin jugadores disponibles.");
        return;
    }
    s.row(Line::styled(
        if wide {
            "  JUGADOR       AGENTE    RANGO      K/D   HS%  KAST%  WR%   ÚLT.5"
        } else {
            "  JUGADOR   AGENTE   RANGO  K/D"
        },
        s.palette.dim,
    ));
    for player in players {
        let marker = if player.is_self { "▶ " } else { "  " };
        let name = roster_identity(player);
        let agent = available_text(&player.agent, "Agente —");
        let rank = available_text(&player.rank, "Rango —");
        let metrics = match &player.stats {
            DataAvailability::Available(stats) => roster_metrics(stats),
            DataAvailability::Hidden
            | DataAvailability::NotAvailable
            | DataAvailability::ApprovalRequired => {
                ["—".into(), "—".into(), "—".into(), "—".into(), "—".into()]
            }
        };
        let style = if player.is_self {
            s.palette.focus
        } else {
            s.palette.base
        };
        let mut spans = vec![
            Span::styled(marker, s.palette.focus),
            Span::styled(cell(&name, if wide { 14 } else { 10 }), style),
            Span::styled(cell(&agent, if wide { 10 } else { 9 }), s.palette.dim),
            Span::styled(cell(&rank, if wide { 11 } else { 8 }), s.palette.rank),
            Span::styled(cell(&metrics[0], if wide { 6 } else { 5 }), style),
        ];
        if wide {
            spans.extend([
                Span::styled(cell(&metrics[1], 6), style),
                Span::styled(cell(&metrics[2], 7), style),
                Span::styled(cell(&metrics[3], 6), style),
                Span::styled(cell(&metrics[4], 6), style),
            ]);
        }
        s.row(Line::from(spans));
    }
}

fn roster_metrics(stats: &HistoricalStats) -> [String; 5] {
    let kd = stats
        .kd_hundredths()
        .map(|value| format!("{}.{:02}", value / 100, value % 100))
        .unwrap_or_else(|| "—".into());
    let percent = |value: Option<u32>| {
        value
            .map(|value| format!("{}.{:01}%", value / 10, value % 10))
            .unwrap_or_else(|| "—".into())
    };
    let hs = percent(stats.headshot_rate_tenths());
    let kast = percent(stats.kast_rate_tenths());
    let wr = percent(stats.win_rate_tenths());
    let recent = if stats.recent.is_empty() {
        "—".into()
    } else {
        stats
            .recent
            .iter()
            .map(|outcome| match outcome {
                MatchOutcome::Win => 'V',
                MatchOutcome::Loss => 'D',
                MatchOutcome::Draw => 'E',
                MatchOutcome::Unknown => '·',
            })
            .collect()
    };
    [kd, hs, kast, wr, recent]
}

fn roster_identity(player: &RosterPlayer) -> String {
    match &player.identity {
        DataAvailability::Available(value) => value.clone(),
        DataAvailability::Hidden => "Jugador oculto".into(),
        DataAvailability::NotAvailable | DataAvailability::ApprovalRequired => {
            format!("Jugador {}", player.slot)
        }
    }
}

fn available_text(value: &DataAvailability<String>, fallback: &str) -> String {
    match value {
        DataAvailability::Available(value) => value.clone(),
        DataAvailability::Hidden
        | DataAvailability::NotAvailable
        | DataAvailability::ApprovalRequired => fallback.into(),
    }
}

fn roster(s: &mut Screen, app: &App, start: usize) {
    let demo = app.demo.as_ref().unwrap();
    let wide = s.width >= 70;
    s.section(
        if start == 0 {
            "ALIADOS · 5 / HISTÓRICO"
        } else {
            "ENEMIGOS · 5 / HISTÓRICO"
        },
        if start == 0 {
            s.palette.good
        } else {
            s.palette.bad
        },
    );
    if start == 0 {
        s.row(Line::styled(
            if wide {
                "  JUGADOR        AGENTE      RANGO     K/D    WR    ÚLT.5  TRK"
            } else {
                "  JUGADOR   AGENTE   RANGO  K/D"
            },
            s.palette.dim,
        ));
    }
    for (index, p) in demo.players.iter().enumerate().skip(start).take(5) {
        let mut spans = vec![
            Span::styled(
                if index == app.player_index {
                    "› "
                } else {
                    "  "
                },
                s.palette.focus,
            ),
            Span::raw(cell(p.name, if wide { 15 } else { 10 })),
            Span::styled(cell(p.agent, if wide { 12 } else { 9 }), s.palette.dim),
            Span::styled(
                cell(p.rank, if wide { 10 } else { 7 }),
                if p.hidden {
                    s.palette.pending
                } else {
                    s.palette.rank
                },
            ),
            Span::raw(cell(p.kd, if wide { 7 } else { 5 })),
        ];
        if wide {
            spans.push(Span::raw(cell(p.wr, 6)));
            for c in p.form.chars() {
                spans.push(Span::styled(
                    c.to_string(),
                    if c == 'V' {
                        s.palette.good
                    } else if c == 'D' {
                        s.palette.bad
                    } else {
                        s.palette.pending
                    },
                ));
            }
            spans.push(Span::styled(
                if p.hidden { "     —" } else { "  [g]" },
                s.palette.dim,
            ));
        }
        s.selected(Line::from(spans), index == app.player_index, app);
    }
}

fn timeline(s: &mut Screen, app: &App) {
    let rounds = &app.demo.as_ref().unwrap().rounds;
    let (kills, deaths) = rounds
        .iter()
        .filter_map(|(_, stats)| *stats)
        .fold((0, 0), |(k, d), (a, b)| (k + a, d + b));
    let capacity = (s.width / 5).max(1);
    let pages = rounds.len().div_ceil(capacity).max(1);
    let page = app.round_page.min(pages - 1);
    s.section(&format!("TUS RONDAS · {kills}K/{deaths}D"), s.palette.focus);
    let visible = rounds
        .iter()
        .skip(page * capacity)
        .take(capacity)
        .collect::<Vec<_>>();
    let mut lines = [vec![], vec![], vec![]];
    for (number, stats) in visible {
        lines[0].push(Span::styled(
            cell(&stats.map_or("—K".into(), |(k, _)| format!("{k}K")), 5),
            if stats.is_some() {
                s.palette.good
            } else {
                s.palette.pending
            },
        ));
        lines[1].push(Span::styled(
            cell(
                &format!("R{number}{}", if stats.is_none() { "*" } else { "" }),
                5,
            ),
            s.palette.dim,
        ));
        lines[2].push(Span::styled(
            cell(&stats.map_or("—D".into(), |(_, d)| format!("{d}D")), 5),
            if stats.is_some() {
                s.palette.bad
            } else {
                s.palette.pending
            },
        ));
    }
    for line in lines {
        s.row(Line::from(line));
    }
    if pages > 1 {
        s.text(format!("[ / ] rondas · {}/{pages}", page + 1));
    }
}

fn demo_postmatch(s: &mut Screen, app: &App) {
    let m = &app.demo.as_ref().unwrap().matches[app.history_index];
    s.section(
        if m.won { "VICTORIA" } else { "DERROTA" },
        if m.won { s.palette.good } else { s.palette.bad },
    );
    s.text(format!(
        "{} · {} / Competitivo / {}",
        m.score, m.map, m.agent
    ));
    s.section("RESULTADO PROPIO", s.palette.focus);
    s.text(format!(
        "K / D / A  {} / {} / {}\nACS        {}\nCambio RR  {:+}",
        m.kills, m.deaths, m.assists, m.acs, m.rr
    ));
    s.text("[p] Volver a partida simulada");
}

fn history(s: &mut Screen, app: &App) {
    s.section("HISTORIAL PROPIO", s.palette.focus);
    if let Some(demo) = &app.demo {
        let wide = s.width >= 70;
        s.text(if wide {
            "  MAPA       AGENTE     RESULTADO   K / D / A    RR"
        } else {
            "  MAPA     V/D    K/D/A       RR"
        });
        for (i, m) in demo.matches.iter().enumerate() {
            let label = format!(
                "{}{}{}{}{}{:+}",
                if i == app.history_index { "› " } else { "  " },
                cell(m.map, if wide { 11 } else { 9 }),
                if wide {
                    cell(m.agent, 10)
                } else {
                    String::new()
                },
                cell(
                    &format!(
                        "{} {}",
                        if m.won { "V" } else { "D" },
                        if wide { m.score } else { "" }
                    ),
                    if wide { 12 } else { 7 }
                ),
                cell(
                    &format!("{}/{}/{}", m.kills, m.deaths, m.assists),
                    if wide { 15 } else { 12 }
                ),
                m.rr
            );
            s.selected(
                Line::styled(label, if m.won { s.palette.good } else { s.palette.bad }),
                i == app.history_index,
                app,
            );
        }
        s.section("RESUMEN", s.palette.focus);
        s.text("2 victorias / 1 derrota / RR +20");
    } else if let Some(entries) = &app.history {
        if entries.is_empty() {
            s.text("No hay partidas recientes.");
        } else {
            s.text("  MODO               CUÁNDO");
            for (i, entry) in entries.iter().enumerate() {
                s.selected(
                    Line::from(format!(
                        "{}{}{}",
                        if i == app.history_index { "› " } else { "  " },
                        cell(&entry.queue, 19),
                        relative_time(entry.started_at_ms)
                    )),
                    i == app.history_index,
                    app,
                );
            }
            if app.detail {
                s.section("PARTIDA SELECCIONADA", s.palette.focus);
                let entry = &entries[app.history_index.min(entries.len() - 1)];
                s.text(format!(
                    "{} / {}\nMapa — · K/D/A — · RR —\nDetalle enriquecido aún no integrado.",
                    entry.queue,
                    relative_time(entry.started_at_ms)
                ));
                s.anchor = Some(s.lines.len().saturating_sub(1));
            }
        }
    } else {
        s.text(if app.history_failed {
            "No se pudo cargar. r reintenta."
        } else {
            "Cargando tus partidas…"
        });
    }
    if app.history_pending {
        s.text("Actualizando…");
    } else if app.history_failed && app.history.is_some() {
        s.text("No se pudo actualizar. Última consulta; r reintenta.");
    }
}

fn settings(s: &mut Screen, app: &App) {
    let settings = &app.settings;
    s.section("APARIENCIA", s.palette.focus);
    s.selected(
        Line::from(format!(
            "{}[t] Tema: {}",
            if settings.selected == 2 { "› " } else { "  " },
            settings.draft.theme.label()
        )),
        settings.selected == 2,
        app,
    );
    s.text("Vista previa; s guarda / r descarta.");
    s.section("GENERAL", s.palette.focus);
    s.selected(
        Line::from(format!(
            "{}Detección: {} s (1–60)",
            if settings.selected == 0 { "› " } else { "  " },
            settings.draft.interval.as_secs()
        )),
        settings.selected == 0,
        app,
    );
    s.selected(
        Line::from(format!(
            "{}Log cambios: [{}]",
            if settings.selected == 1 { "› " } else { "  " },
            if settings.draft.log_transitions {
                "x"
            } else {
                " "
            }
        )),
        settings.selected == 1,
        app,
    );
    s.text(settings.status());
    s.section("PRIVACIDAD", s.palette.focus);
    s.text("Identidad oculta: respetar\nDatos ausentes: —\nSin memoria ni controles del juego.");
    s.section("IMÁGENES", s.palette.focus);
    s.text("Retratos opcionales: pendientes.\nLa interfaz funciona sin imágenes.");
    if app.demo.is_some() {
        s.text("DEMO: guardar solo cambia esta sesión.");
    }
}

fn phase_label(app: &App) -> &'static str {
    if let Some(demo) = &app.demo {
        return if demo.post {
            "Postpartida simulada"
        } else {
            "En partida simulada"
        };
    }
    match app.state.as_ref().map(|v| v.phase) {
        None | Some(GamePhase::Unknown) => "Conectando…",
        Some(GamePhase::GameOpen) => "VALORANT abierto · esperando partida",
        Some(phase) => phase.label(),
    }
}

pub(super) fn render(area: Rect, frame: &mut ratatui::Frame<'_>, app: &mut App) {
    let palette = Palette::new(app.settings.draft.theme);
    frame.render_widget(Block::default().style(palette.base), area);
    if area.width < 38 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("Amplía la terminal a 38×10.\nq salir").style(palette.pending),
            area,
        );
        return;
    }
    if let Some(demo) = &app.demo {
        let capacity = ((area.width - 2) as usize / 5).max(1);
        app.round_page = app
            .round_page
            .min(demo.rounds.len().div_ceil(capacity).saturating_sub(1));
    }
    let mut screen = content(app, area.width - 2);
    let tab_rows = if area.width < 72 { 2 } else { 1 };
    let footer_rows = if area.width < 72 { 2 } else { 1 };
    let body = Rect::new(
        area.x + 1,
        area.y + 2 + tab_rows,
        area.width - 2,
        area.height.saturating_sub(4 + tab_rows + footer_rows),
    );
    let max_scroll = screen
        .lines
        .len()
        .saturating_sub(body.height as usize)
        .min(u16::MAX as usize) as u16;
    app.scroll = app.scroll.min(max_scroll);
    if app.follow_selection {
        if let Some(anchor) = screen.anchor {
            let anchor = anchor.min(u16::MAX as usize) as u16;
            if anchor < app.scroll {
                app.scroll = anchor;
            } else if anchor >= app.scroll.saturating_add(body.height) {
                app.scroll = anchor
                    .saturating_sub(body.height.saturating_sub(1))
                    .min(max_scroll);
            }
        }
        app.follow_selection = false;
    }
    let status = if app.demo.is_some() {
        "DEMO · FICTICIO"
    } else {
        "SOLO LECTURA"
    };
    let bottom = if max_scroll > 0 {
        format!(" PgUp/PgDn · {}/{} ", app.scroll + 1, max_scroll + 1)
    } else {
        String::new()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(palette.border)
        .title(Line::styled(
            format!(" VTRACKER · {status} "),
            palette.focus,
        ))
        .title_bottom(bottom);
    frame.render_widget(block, area);
    for y in [body.y - 1, body.bottom()]
        .into_iter()
        .chain(screen.sections.iter().filter_map(|line| {
            let line = *line as u16;
            (line >= app.scroll && line < app.scroll.saturating_add(body.height))
                .then(|| body.y + line - app.scroll)
        }))
    {
        frame.render_widget(
            Paragraph::new("├").style(palette.border),
            Rect::new(area.x, y, 1, 1),
        );
        frame.render_widget(
            Paragraph::new("┤").style(palette.border),
            Rect::new(area.right() - 1, y, 1, 1),
        );
    }
    for row in 0..tab_rows {
        let indices = if tab_rows == 1 {
            0..5
        } else if row == 0 {
            0..3
        } else {
            3..5
        };
        let tabs = indices
            .map(|i| {
                Span::styled(
                    format!(" {} {} ", i + 1, TABS[i]),
                    if i == app.selected_tab {
                        if app.focus == Focus::Tabs {
                            palette
                                .selected
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        } else {
                            palette.focus
                        }
                    } else {
                        palette.dim
                    },
                )
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(Line::from(tabs)),
            Rect::new(area.x + 1, area.y + 1 + row, area.width - 2, 1),
        );
    }
    let separator = Line::styled("─".repeat((area.width - 2) as usize), palette.border);
    frame.render_widget(
        Paragraph::new(separator.clone()),
        Rect::new(area.x + 1, body.y - 1, area.width - 2, 1),
    );
    frame.render_widget(
        Paragraph::new(std::mem::take(&mut screen.lines))
            .scroll((app.scroll, 0))
            .style(palette.base),
        body,
    );
    frame.render_widget(
        Paragraph::new(separator),
        Rect::new(area.x + 1, body.bottom(), area.width - 2, 1),
    );
    let hint = if app.focus == Focus::Tabs {
        "←/→ vista · Enter contenido"
    } else {
        match app.selected_tab {
            0 => "Enter partida",
            1 if app.demo.as_ref().is_some_and(|d| d.post) => "p partida · Esc volver",
            1 if app.demo.is_some() => "↑↓ jugador · Enter · g TRK",
            3 => "↑↓ partida · Enter detalle · r",
            4 => "↑↓ +/- · s guardar · r descartar",
            _ => "r actualizar · PgUp/PgDn",
        }
    };
    let controls = if app.demo.is_some() && app.selected_tab == 1 {
        "1–5 Tab t · p fase · Esc · q salir"
    } else {
        "1–5 Tab · t tema · Esc · q salir"
    };
    let footer = if footer_rows == 1 {
        format!("{hint} · {controls}")
    } else {
        format!(
            "{}\n{}",
            cell(hint, (area.width - 2) as usize).trim_end(),
            controls
        )
    };
    frame.render_widget(
        Paragraph::new(footer).style(palette.dim),
        Rect::new(area.x + 1, body.bottom() + 1, area.width - 2, footer_rows),
    );
}

#[cfg(test)]
mod tests {
    use super::super::{demo::Demo, worker::Worker};
    use super::*;
    use crate::{
        config::{Config, Theme},
        game::GameState,
        providers::{StateInfo, capabilities::Confidence, live_match::LiveMatchContext},
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    fn demo_app() -> App {
        let mut app = App::new(&Config::default());
        app.demo = Some(Demo::default());
        app.selected_tab = 1;
        app
    }

    fn snapshot(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame.area(), frame, app))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .chunks(width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn key(app: &mut App, worker: &Worker, code: KeyCode) {
        app.key(KeyEvent::new(code, KeyModifiers::NONE), worker);
    }

    #[test]
    fn mockup_fits_target_sizes_with_all_ten_players_and_pending_round() {
        for (width, height) in [(72, 24), (38, 26)] {
            let mut app = demo_app();
            let text = snapshot(&mut app, width, height);
            println!("\n{width}x{height}\n{text}");
            for p in &app.demo.as_ref().unwrap().players {
                assert!(text.contains(p.name), "missing {}", p.name);
            }
            assert!(text.find("ALIADOS").unwrap() < text.find("TUS RONDAS").unwrap());
            assert!(text.find("TUS RONDAS").unwrap() < text.find("ENEMIGOS").unwrap());
            assert!(
                text.contains("8K/4D")
                    && text.contains("—K")
                    && text.contains("—D")
                    && text.contains("R7*")
            );
            assert!(text.contains("q salir"));
            assert_eq!(app.scroll, 0);
            assert!(text.contains("DEMO · FICTICIO"));
        }
    }

    #[test]
    fn every_view_and_theme_fits_cells_and_handles_tiny_terminals() {
        let mut app = demo_app();
        for theme in [Theme::System, Theme::Dark, Theme::Light, Theme::Mono] {
            app.settings.draft.theme = theme;
            for tab in 0..5 {
                app.select_tab(tab);
                for width in [36, 70, 98] {
                    let screen = content(&app, width);
                    assert!(
                        screen
                            .lines
                            .iter()
                            .all(|line| line.width() <= width as usize)
                    );
                }
                for (width, height) in [(1, 1), (30, 8), (38, 10), (38, 26), (72, 24), (120, 40)] {
                    let text = snapshot(&mut app, width, height);
                    if width >= 38 {
                        assert!(text.contains("q salir"));
                    }
                }
            }
        }
    }

    #[test]
    fn selection_scrolls_into_view_and_escape_closes_detail_without_quitting() {
        let worker = Worker::demo().unwrap();
        let mut app = demo_app();
        for _ in 0..9 {
            key(&mut app, &worker, KeyCode::Down);
        }
        assert_eq!(app.player_index, 9);
        let text = snapshot(&mut app, 38, 14);
        assert!(text.contains("Ámbar") && app.scroll > 0);
        key(&mut app, &worker, KeyCode::Enter);
        assert!(snapshot(&mut app, 38, 14).contains("ADR 129"));
        key(&mut app, &worker, KeyCode::Esc);
        assert!(!app.detail && !app.should_quit);
        key(&mut app, &worker, KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn hidden_demo_profile_has_no_stats_or_external_link() {
        let worker = Worker::demo().unwrap();
        let mut app = demo_app();
        app.player_index = 6;
        key(&mut app, &worker, KeyCode::Char('g'));
        let text = snapshot(&mut app, 72, 32);
        assert!(text.contains("Identidad y estadísticas ocultas"));
        assert!(text.contains("identidad oculta; sin enlace"));
        assert!(!text.contains("HS ") && !text.contains("https://"));
    }

    #[test]
    fn live_data_never_inherits_demo_roster_or_round_numbers() {
        let mut app = App::new(&Config::default());
        app.selected_tab = 1;
        app.update_state(StateInfo::new(
            GamePhase::InMatch,
            GameState::GameOpen,
            Confidence::High,
            "secret-source",
            true,
            true,
        ));
        app.live_match = Some(LiveMatchContext {
            map: "Ascent".into(),
            mode: "Bomb".into(),
            agent: Some("Sova".into()),
            roster: None,
        });
        let text = snapshot(&mut app, 72, 24);
        assert!(text.contains("Sova") && text.contains("no disponible") && text.contains("—K"));
        for absent in ["Norte", "4:2", "R7*", "secret-source", "Confianza", "DEMO"] {
            assert!(!text.contains(absent));
        }
        for mode in ["Deathmatch", "HURM", "Escalation"] {
            app.live_match.as_mut().unwrap().mode = mode.into();
            let text = snapshot(&mut app, 72, 24);
            assert!(text.contains("TU PARTICIPACIÓN"));
            assert!(!text.contains("ENEMIGOS") && !text.contains("TUS RONDAS"));
        }
        let completed = crate::providers::match_detail::CompletedMatch {
            own_puuid: "not-for-display".into(),
            rounds: None,
            summary: Some(crate::providers::match_detail::MatchSummary {
                mode: crate::models::GameMode::Deathmatch,
                stats: crate::models::PlayerMatchStats {
                    kills: 2,
                    deaths: 1,
                    assists: 0,
                    ..Default::default()
                },
            }),
        };
        let summary = super::super::completed_match_content(&completed);
        assert!(summary.ends_with("2  1  0  —"));
        assert!(!summary.contains("not-for-display"));
    }

    #[test]
    fn real_roster_renders_visible_and_hidden_players_without_identifiers() {
        let mut app = App::new(&Config::default());
        app.selected_tab = 1;
        app.update_state(StateInfo::new(
            GamePhase::InMatch,
            GameState::GameOpen,
            Confidence::High,
            "local-websocket",
            true,
            true,
        ));
        let player = |side: RosterSide,
                      slot: u8,
                      is_self: bool,
                      identity: DataAvailability<String>,
                      agent: &str,
                      rank: &str| {
            let stats = if matches!(identity, DataAvailability::Hidden) {
                DataAvailability::Available(HistoricalStats {
                    matches: 2,
                    decided_matches: 2,
                    wins: 1,
                    kills: 3,
                    deaths: 2,
                    headshots: 2,
                    bodyshots: 2,
                    kast_rounds: 2,
                    rounds_played: 3,
                    recent: vec![MatchOutcome::Win, MatchOutcome::Loss],
                    ..Default::default()
                })
            } else {
                DataAvailability::NotAvailable
            };
            RosterPlayer {
                side,
                slot,
                is_self,
                identity,
                agent: DataAvailability::Available(agent.into()),
                rank: DataAvailability::Available(rank.into()),
                stats,
            }
        };
        app.live_match = Some(LiveMatchContext {
            map: "Haven".into(),
            mode: "Bomb".into(),
            agent: Some("Omen".into()),
            roster: Some(
                RosterSnapshot::new(vec![
                    player(
                        RosterSide::Ally,
                        1,
                        true,
                        DataAvailability::Available("Tú".into()),
                        "Omen",
                        "Diamante 1",
                    ),
                    player(
                        RosterSide::Enemy,
                        1,
                        false,
                        DataAvailability::Hidden,
                        "Jett",
                        "Radiante",
                    ),
                ])
                .unwrap(),
            ),
        });

        let text = snapshot(&mut app, 72, 24);
        assert!(text.contains("Tú") && text.contains("Omen") && text.contains("Diamante 1"));
        assert!(
            text.contains("Jugador oculto") && text.contains("Jett") && text.contains("Radiante")
        );
        assert!(text.contains("K/D") && text.contains("HS%") && text.contains("KAST%"));
        assert!(text.contains("1.50") && text.contains("50.0%") && text.contains("VD"));
        assert!(!text.contains("puuid") && !text.contains("local-websocket"));
    }

    #[test]
    fn navigation_focus_and_demo_history_follow_the_mockup() {
        let worker = Worker::demo().unwrap();
        let mut app = demo_app();
        key(&mut app, &worker, KeyCode::Char('4'));
        key(&mut app, &worker, KeyCode::Down);
        key(&mut app, &worker, KeyCode::Enter);
        let text = snapshot(&mut app, 72, 24);
        assert!(text.contains("DERROTA") && text.contains("Haven"));
        key(&mut app, &worker, KeyCode::Esc);
        assert!(!app.demo.as_ref().unwrap().post);
        key(&mut app, &worker, KeyCode::Char('5'));
        key(&mut app, &worker, KeyCode::Tab);
        let draft = app.settings.draft.clone();
        key(&mut app, &worker, KeyCode::Char('+'));
        assert_eq!(draft, app.settings.draft);
        key(&mut app, &worker, KeyCode::Left);
        assert_eq!(app.selected_tab, 3);
        key(&mut app, &worker, KeyCode::Enter);
        assert!(app.focus == Focus::Content);
        app.schedule_data(&worker);
        assert!(!app.history_pending && !app.context_pending);
    }

    #[test]
    fn long_timelines_paginate_and_unicode_uses_terminal_cell_width() {
        let mut app = demo_app();
        app.demo.as_mut().unwrap().rounds = (1..=31).map(|n| (n, Some((0, 2)))).collect();
        app.round_page = usize::MAX;
        let text = snapshot(&mut app, 38, 27);
        assert!(text.contains("R31") && text.contains("5/5"));
        assert_eq!(app.round_page, 4);
        let worker = Worker::demo().unwrap();
        key(&mut app, &worker, KeyCode::Char('['));
        assert!(snapshot(&mut app, 38, 27).contains("4/5"));
        assert_eq!(Span::raw(cell("界界界", 5)).width(), 5);
        assert_eq!(cell("a\u{301}bc", 2), "a\u{301}b");
        assert!(!cell("\x1b\n\tname", 20).contains(['\x1b', '\n', '\t']));
    }

    #[test]
    fn monochrome_render_uses_no_explicit_colors() {
        let mut app = demo_app();
        app.settings.draft.theme = Theme::Mono;
        let mut terminal = Terminal::new(TestBackend::new(72, 24)).unwrap();
        terminal
            .draw(|frame| render(frame.area(), frame, &mut app))
            .unwrap();
        for cell in &terminal.backend().buffer().content {
            assert_eq!(cell.fg, ratatui::style::Color::Reset);
            assert_eq!(cell.bg, ratatui::style::Color::Reset);
        }
    }
}
