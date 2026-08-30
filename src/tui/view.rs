//! Composición de las vistas en celdas de terminal; sin I/O.
use ratatui::{
    layout::{Alignment, Rect},
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
    pub setting_rows: [Option<usize>; 3],
    sections: Vec<usize>,
    width: usize,
    palette: Palette,
}

impl Screen {
    fn new(width: u16, palette: Palette) -> Self {
        Self {
            lines: vec![],
            anchor: None,
            setting_rows: [None; 3],
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

    fn setting(&mut self, index: usize, line: Line<'static>, app: &App) {
        self.setting_rows[index] = Some(self.lines.len());
        self.selected(line, app.settings.selected == index, app);
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
    s.section("MI PERFIL", s.palette.focus);
    profile_summary(s, app);
    s.section("ÚLTIMAS 5 RANKED", s.palette.focus);
    if let Some(demo) = &app.demo {
        let kills: u32 = demo.matches.iter().map(|m| m.kills).sum();
        let deaths: u32 = demo.matches.iter().map(|m| m.deaths).sum();
        s.text(format!(
            "3 partidas · 2 V / 1 D · K/D {:.2} · HS 26.0%\n{kills} kills / {deaths} muertes",
            kills as f64 / deaths as f64
        ));
    } else if let Some(summary) = history_summary(app) {
        s.text(summary);
    } else if app.history_pending {
        s.text("Cargando resultados y rendimiento…");
    } else if app.history_failed {
        s.text("No se pudo actualizar el historial · r para reintentar");
    } else {
        s.text("Aún no hay partidas disponibles en esta sesión.");
    }
    s.section(match_section_title(app), s.palette.focus);
    s.row(Line::styled(phase_label(app), phase_style(s, app)));
    if let Some(context) = &app.live_match {
        s.text(format!(
            "{} · {} · {}",
            context.map,
            context.mode,
            context.agent.as_deref().unwrap_or("Agente no disponible")
        ));
        if let Some(roster) = &context.roster {
            let unnamed = roster
                .players
                .iter()
                .filter(|player| !matches!(player.identity, DataAvailability::Available(_)))
                .count();
            s.text(format!(
                "{} jugadores · {} {} sin nombre público",
                roster.players.len(),
                unnamed,
                plural(unnamed, "perfil", "perfiles")
            ));
        }
    } else if let Some(completed) = &app.completed_match {
        s.text(format!(
            "Última: {} · {} · {}",
            outcome_text(completed.outcome),
            completed.map,
            title_text(completed.mode.label())
        ));
    } else if app
        .state
        .as_ref()
        .is_some_and(|state| state.client_found && state.phase == GamePhase::Idle)
    {
        s.text("Tu perfil seguirá actualizándose mientras esperas.");
    }
}

fn profile_summary(s: &mut Screen, app: &App) {
    if app.demo.is_some() {
        let rank = "DIAMANTE 2";
        s.row(Line::from(vec![
            Span::styled(rank, s.palette.rank_style(rank)),
            Span::styled(" · Nivel 142", s.palette.base),
        ]));
        s.row(rr_progress_line(64, &s.palette, rank));
    } else {
        match (&app.own_profile, &app.competitive) {
            (Some(profile), Some(rank)) => {
                let rank_label = title_text(&crate::ui::competitive_tier_label(rank.tier));
                s.row(Line::from(vec![
                    Span::styled(rank_label.clone(), s.palette.rank_style(&rank_label)),
                    Span::styled(format!(" · Nivel {}", profile.level), s.palette.base),
                ]));
                let progress = rr_progress_line(rank.ranked_rating, &s.palette, &rank_label);
                s.row(progress);
                if rank.games > 0 {
                    s.text(format!(
                        "{} XP · {} victorias / {} competitivas",
                        profile.xp, rank.wins, rank.games
                    ));
                } else {
                    s.text(format!(
                        "{} XP · rango confirmado por tu última Ranked",
                        profile.xp
                    ));
                }
            }
            (Some(profile), None) => {
                s.text(format!("Nivel {} · {} XP", profile.level, profile.xp));
                s.text("Sin rango competitivo disponible.");
            }
            (None, _) if app.profile_pending => {
                s.text("Cargando tu perfil, rango y RR…");
            }
            (None, _) if app.profile_failed => {
                s.text("No se pudo cargar tu perfil · r para reintentar");
            }
            (None, _) => {
                s.text("Abre Riot Client para cargar tu perfil.");
            }
        }
        if app.own_profile.is_none()
            && let Some(rank) = &app.competitive
        {
            let rank_label = title_text(&crate::ui::competitive_tier_label(rank.tier));
            s.row(Line::styled(
                rank_label.clone(),
                s.palette.rank_style(&rank_label),
            ));
            let progress = rr_progress_line(rank.ranked_rating, &s.palette, &rank_label);
            s.row(progress);
        }
    }
}

fn rr_progress_line(rr: u32, palette: &Palette, rank: &str) -> Line<'static> {
    const CELLS: usize = 10;
    let rr = rr.min(100);
    let filled = rr as usize * CELLS / 100;
    let rank_style = palette.rank_style(rank);
    Line::from(vec![
        Span::styled("RR ", palette.base),
        Span::styled("█".repeat(filled), rank_style),
        Span::styled("░".repeat(CELLS - filled), palette.border),
        Span::styled(format!(" {rr} / 100"), rank_style),
    ])
}

fn match_section_title(app: &App) -> &'static str {
    match app.state.as_ref().map(|state| state.phase) {
        Some(GamePhase::InMatch) => "PARTIDA EN CURSO",
        Some(GamePhase::PreGame | GamePhase::AgentSelect) => "PREPARANDO PARTIDA",
        Some(GamePhase::PostMatch) => "ÚLTIMA PARTIDA",
        _ => "ESTADO DE PARTIDA",
    }
}

fn phase_style(s: &Screen, app: &App) -> Style {
    match app.state.as_ref().map(|state| state.phase) {
        Some(GamePhase::InMatch | GamePhase::PreGame | GamePhase::AgentSelect) => s.palette.good,
        Some(GamePhase::ClientClosed) => s.palette.pending,
        _ => s.palette.base,
    }
}

fn profile(s: &mut Screen, app: &App) {
    s.section("MI PERFIL", s.palette.focus);
    profile_summary(s, app);
    s.section("ÚLTIMAS 5 RANKED", s.palette.focus);
    if app.demo.is_some() {
        s.text("Competitivo / últimas 20 partidas\nK/D 1.18 · WR 55% · HS 26%\n11 victorias / 9 derrotas");
        s.section("POR AGENTE", s.palette.focus);
        s.text("AGENTE     PJ    K/D    WR\nSova       12    1.24   58%\nOmen        5    1.08   40%\nKilljoy     3    1.11   67%");
    } else if let Some(summary) = history_summary(app) {
        s.text(summary);
    } else if app.history_pending {
        s.text("Calculando tu rendimiento reciente…");
    } else {
        s.text("No hay detalles recientes disponibles.");
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
    } else if app.profile_pending {
        s.text("Cargando cambios competitivos…");
    } else if app.competitive_updates.is_empty() {
        s.text("No hay cambios competitivos recientes.");
    } else {
        for update in &app.competitive_updates {
            s.row(Line::styled(
                format!(
                    "{:+} RR · {}{}",
                    update.rr_earned,
                    crate::ui::competitive_tier_label(update.tier_after),
                    if update.performance_bonus > 0 {
                        format!(" · +{} rendimiento", update.performance_bonus)
                    } else {
                        String::new()
                    }
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
        postmatch(s, completed);
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
    let preparing = app
        .state
        .as_ref()
        .is_some_and(|state| matches!(state.phase, GamePhase::PreGame | GamePhase::AgentSelect));
    if preparing {
        s.section("COMPAÑEROS · SELECCIÓN DE AGENTE", s.palette.good);
        if let Some(roster) = &context.roster {
            live_roster(s, app, roster, RosterSide::Ally);
        } else {
            s.text("El equipo todavía no está disponible.");
        }
        s.row(Line::styled(
            "Los rivales aparecen al finalizar la selección.",
            s.palette.dim,
        ));
        if app.detail
            && let Some(roster) = &context.roster
            && let Some(player) = roster.players.get(app.player_index)
        {
            live_player_detail(s, app, roster, player);
        }
        return;
    }
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
                live_roster(s, app, roster, RosterSide::Participant);
            } else {
                s.section("ALIADOS", s.palette.good);
                live_roster(s, app, roster, RosterSide::Ally);
                s.section("ENEMIGOS", s.palette.bad);
                live_roster(s, app, roster, RosterSide::Enemy);
            }
        }
    } else {
        s.section("ALIADOS", s.palette.good);
        if let Some(roster) = &context.roster {
            live_roster(s, app, roster, RosterSide::Ally);
        } else {
            s.text(format!(
                "Tú / {}\nRoster todavía no disponible.",
                context.agent.as_deref().unwrap_or("—"),
            ));
        }
        s.section("TUS RONDAS", s.palette.focus);
        s.row(Line::styled("—K   /   —D", s.palette.pending));
        s.text("Kills y muertes por ronda aún no integradas.");
        s.section("ENEMIGOS", s.palette.bad);
        if let Some(roster) = &context.roster {
            live_roster(s, app, roster, RosterSide::Enemy);
        } else {
            s.text("Roster todavía no disponible.");
        }
    }
    if app.detail
        && let Some(roster) = &context.roster
        && let Some(player) = roster.players.get(app.player_index)
    {
        live_player_detail(s, app, roster, player);
    }
}

fn postmatch(s: &mut Screen, completed: &super::PostMatch) {
    let result_style = match completed.outcome {
        MatchOutcome::Win => s.palette.good,
        MatchOutcome::Loss => s.palette.bad,
        MatchOutcome::Draw | MatchOutcome::Unknown => s.palette.focus,
    };
    s.section(outcome_text(completed.outcome), result_style);
    let score = score_text(completed.own_score, completed.opponent_score);
    s.row(Line::styled(
        format!(
            "{}{} · {} · {}",
            score.map_or_else(String::new, |score| format!("{score} · ")),
            completed.map,
            title_text(completed.mode.label()),
            completed.agent
        ),
        result_style,
    ));
    s.section("TU RESULTADO", s.palette.focus);
    s.text(format!(
        "K / D / A    {} / {} / {}\nK/D            {}\nPuntos         {}",
        completed.stats.kills,
        completed.stats.deaths,
        completed.stats.assists,
        kd_text(completed.stats.kills, completed.stats.deaths),
        completed
            .stats
            .combat_score
            .map_or_else(|| "No disponible".into(), |score| score.to_string())
    ));
    if !completed.rounds.is_empty() {
        s.section(
            &format!("TUS RONDAS · {} JUGADAS", completed.rounds.len()),
            s.palette.focus,
        );
        s.row(Line::styled(
            "RONDA   CIERRE             K   D",
            s.palette.dim,
        ));
        for round in &completed.rounds {
            s.row(Line::from(format!(
                "{:>3}     {:<18} {:>1}   {:>1}",
                round.number, round.result, round.kills, round.deaths
            )));
        }
    } else {
        s.section("DETALLE", s.palette.focus);
        s.text("Este modo no utiliza rondas con Spike.");
    }
}

fn live_roster(s: &mut Screen, app: &App, roster: &RosterSnapshot, side: RosterSide) {
    let wide = s.width >= 70;
    let player_context = s.width >= 90;
    let tracker_column = s.width >= if player_context { 93 } else { 78 };
    let players = roster
        .players
        .iter()
        .enumerate()
        .filter(|(_, player)| player.side == side)
        .collect::<Vec<_>>();
    if players.is_empty() {
        s.text("Sin jugadores disponibles.");
        return;
    }
    let mut header = vec![
        Span::styled("  ", s.palette.dim),
        Span::styled("  ", s.palette.dim),
        Span::styled(cell("JUGADOR", if wide { 16 } else { 10 }), s.palette.dim),
        Span::styled(if wide { "  " } else { " " }, s.palette.dim),
        Span::styled(cell("AGENTE", if wide { 11 } else { 9 }), s.palette.dim),
        Span::styled(" ", s.palette.dim),
        Span::styled(cell("RANGO", if wide { 11 } else { 8 }), s.palette.dim),
    ];
    if player_context {
        header.push(Span::styled(cell("NIV.", 6), s.palette.dim));
    }
    header.push(Span::styled(
        cell("K/D", if wide { 6 } else { 5 }),
        s.palette.dim,
    ));
    if wide {
        header.extend([
            Span::styled(cell("HS%", 6), s.palette.dim),
            Span::styled(cell("KAST%", 7), s.palette.dim),
            Span::styled(cell("WR%", 6), s.palette.dim),
            Span::styled(cell("ÚLT.5", 6), s.palette.dim),
        ]);
        if tracker_column {
            header.push(Span::styled("TRK", s.palette.dim));
        }
    }
    s.row(Line::from(header));
    for (index, player) in players {
        let marker = if player.is_self { "▶ " } else { "  " };
        let name = roster_identity(player);
        let agent = available_text(&player.agent, "Agente —");
        let rank = available_text(&player.rank, "Rango —");
        let level = roster_level(app, player);
        let premade = premade_marker(roster, player);
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
            Span::styled(
                premade,
                s.palette.premade_style(available_premade_label(player)),
            ),
            Span::styled(cell(&name, if wide { 16 } else { 10 }), style),
            Span::raw(if wide { "  " } else { " " }),
            Span::styled(cell(&agent, if wide { 11 } else { 9 }), s.palette.dim),
            Span::raw(" "),
            Span::styled(
                cell(&rank, if wide { 11 } else { 8 }),
                s.palette.rank_style(&rank),
            ),
        ];
        if player_context {
            spans.push(Span::styled(cell(&level, 6), s.palette.dim));
        }
        spans.push(Span::styled(
            cell(&metrics[0], if wide { 6 } else { 5 }),
            style,
        ));
        if wide {
            spans.extend([
                Span::styled(cell(&metrics[1], 6), s.palette.focus),
                Span::styled(cell(&metrics[2], 7), s.palette.focus),
                Span::styled(cell(&metrics[3], 6), style),
            ]);
            let recent = stats_recent(player);
            for outcome in recent {
                spans.push(Span::styled(
                    outcome_short(*outcome),
                    match outcome {
                        MatchOutcome::Win => s.palette.good,
                        MatchOutcome::Loss => s.palette.bad,
                        MatchOutcome::Draw => s.palette.pending,
                        MatchOutcome::Unknown => s.palette.dim,
                    },
                ));
            }
            let recent_count = recent.len();
            spans.push(Span::raw(" ".repeat(6_usize.saturating_sub(recent_count))));
            if tracker_column {
                spans.push(Span::styled(
                    if super::tracker_url(player).is_some() {
                        "[↗]"
                    } else {
                        " — "
                    },
                    if super::tracker_url(player).is_some() {
                        s.palette.focus
                    } else {
                        s.palette.dim
                    },
                ));
            }
        }
        s.selected(Line::from(spans), index == app.player_index, app);
    }
}

fn stats_recent(player: &RosterPlayer) -> &[MatchOutcome] {
    match &player.stats {
        DataAvailability::Available(stats) => &stats.recent,
        DataAvailability::Hidden
        | DataAvailability::NotAvailable
        | DataAvailability::ApprovalRequired => &[],
    }
}

fn live_player_detail(s: &mut Screen, app: &App, roster: &RosterSnapshot, player: &RosterPlayer) {
    let name = roster_identity(player);
    let agent = available_text(&player.agent, "Agente —");
    s.section(&format!("{name} / {agent}"), s.palette.focus);
    let rank = available_text(&player.rank, "Rango —");
    let level = roster_level(app, player);
    let premade = premade_detail(roster, player);
    s.row(Line::from(vec![
        Span::raw("Nivel "),
        Span::styled(level, s.palette.focus),
        Span::raw(" · Premade "),
        Span::styled(
            premade,
            s.palette.premade_style(available_premade_label(player)),
        ),
    ]));
    if let DataAvailability::Available(stats) = &player.stats {
        let metrics = roster_metrics(stats);
        s.row(Line::from(vec![
            Span::styled(format!("{rank} · "), s.palette.rank_style(&rank)),
            Span::raw(format!(
                "{} Ranked · K/D {} · WR {}",
                stats.matches, metrics[0], metrics[3]
            )),
        ]));
        s.text(format!(
            "HS {} · KAST {} · forma {}",
            metrics[1], metrics[2], metrics[4]
        ));
    } else {
        s.row(Line::styled(
            format!("{rank} · estadísticas Ranked no disponibles"),
            s.palette.pending,
        ));
    }
    let has_tracker = super::tracker_url(player).is_some();
    let tracker = if app.tracker_notice {
        if !has_tracker {
            "Tracker.gg no disponible: falta un Riot ID público."
        } else if app.tracker_open_failed {
            "No se pudo abrir Tracker.gg en el navegador."
        } else {
            "Tracker.gg abierto en el navegador."
        }
    } else if has_tracker {
        "[g] Abrir perfil en Tracker.gg"
    } else {
        "Tracker.gg no disponible para este jugador."
    };
    s.row(Line::styled(
        tracker,
        if has_tracker {
            s.palette.focus
        } else {
            s.palette.dim
        },
    ));
    s.anchor = Some(s.lines.len().saturating_sub(1));
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
        DataAvailability::Hidden => format!("Jugador {}", player.slot),
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

fn available_number(value: &DataAvailability<u32>) -> String {
    match value {
        DataAvailability::Available(value) if *value > 0 => value.to_string(),
        DataAvailability::Hidden => "priv.".into(),
        DataAvailability::Available(_)
        | DataAvailability::NotAvailable
        | DataAvailability::ApprovalRequired => "—".into(),
    }
}

fn roster_level(app: &App, player: &RosterPlayer) -> String {
    if player.is_self
        && let Some(profile) = &app.own_profile
        && profile.level > 0
    {
        return profile.level.to_string();
    }
    available_number(&player.level)
}

fn available_premade_label(player: &RosterPlayer) -> &str {
    match &player.premade {
        DataAvailability::Available(label) => label,
        DataAvailability::Hidden
        | DataAvailability::NotAvailable
        | DataAvailability::ApprovalRequired => "—",
    }
}

fn premade_size(roster: &RosterSnapshot, label: &str) -> usize {
    roster
        .players
        .iter()
        .filter(|player| {
            matches!(&player.premade, DataAvailability::Available(other) if other == label)
        })
        .count()
}

fn premade_marker(roster: &RosterSnapshot, player: &RosterPlayer) -> &'static str {
    let label = available_premade_label(player);
    if label == "Solo" || label == "—" || premade_size(roster, label) < 2 {
        return "  ";
    }
    "• "
}

fn premade_detail(roster: &RosterSnapshot, player: &RosterPlayer) -> String {
    let label = available_premade_label(player);
    if label == "Solo" {
        return "Solo".into();
    }
    if label == "—" {
        return "sin dato".into();
    }
    let size = premade_size(roster, label);
    format!(
        "{label} · {size} {}",
        if size == 1 { "jugador" } else { "jugadores" }
    )
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
                    s.palette.rank_style(p.rank)
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
    s.section("HISTORIAL RANKED", s.palette.focus);
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
            let wide = s.width >= 70;
            s.row(Line::styled(
                if wide {
                    "  RES.      MAPA         AGENTE      K / D / A     HS%     CUÁNDO"
                } else {
                    "  RES.  MAPA       K/D/A       CUÁNDO"
                },
                s.palette.dim,
            ));
            for (i, item) in entries.iter().enumerate() {
                let details = item.details.as_ref();
                let result = details.map_or("—".into(), |details| {
                    let score =
                        score_text(details.own_score, details.opponent_score).unwrap_or_default();
                    format!("{} {score}", outcome_short(details.outcome))
                });
                let map = details.map_or("Sin detalle", |details| details.map.as_str());
                let agent = details.map_or("—", |details| details.agent.as_str());
                let kda = details.map_or("—".into(), |details| {
                    format!(
                        "{}/{}/{}",
                        details.stats.kills, details.stats.deaths, details.stats.assists
                    )
                });
                let hs =
                    details.map_or_else(|| "—".into(), |details| match_hs_text(&details.stats));
                let mut row = format!(
                    "{}{}{}",
                    if i == app.history_index { "› " } else { "  " },
                    cell(&result, if wide { 10 } else { 6 }),
                    cell(map, if wide { 13 } else { 11 }),
                );
                if wide {
                    row.push_str(&cell(agent, 12));
                }
                row.push_str(&cell(&kda, if wide { 14 } else { 12 }));
                if wide {
                    row.push_str(&cell(&hs, 8));
                }
                row.push_str(&relative_time(item.entry.started_at_ms));
                let style = details.map_or(s.palette.base, |details| match details.outcome {
                    MatchOutcome::Win => s.palette.good,
                    MatchOutcome::Loss => s.palette.bad,
                    MatchOutcome::Draw | MatchOutcome::Unknown => s.palette.base,
                });
                s.selected(Line::styled(row, style), i == app.history_index, app);
            }
            if app.detail {
                let item = &entries[app.history_index.min(entries.len() - 1)];
                if let Some(details) = &item.details {
                    let outcome_style = match details.outcome {
                        MatchOutcome::Win => s.palette.good,
                        MatchOutcome::Loss => s.palette.bad,
                        MatchOutcome::Draw | MatchOutcome::Unknown => s.palette.pending,
                    };
                    s.section(outcome_text(details.outcome), outcome_style);
                    let score = score_text(details.own_score, details.opponent_score)
                        .unwrap_or_else(|| "Marcador no disponible".into());
                    s.row(Line::from(vec![
                        Span::styled(score, outcome_style.add_modifier(Modifier::BOLD)),
                        Span::styled(format!("  {}", details.map), s.palette.base),
                    ]));
                    s.text(format!(
                        "{} · Ranked · {}",
                        details.agent,
                        relative_time(item.entry.started_at_ms)
                    ));
                    s.section("TU RENDIMIENTO", s.palette.focus);
                    s.row(Line::from(vec![
                        Span::raw("Kills "),
                        Span::styled(details.stats.kills.to_string(), s.palette.good),
                        Span::raw("   Muertes "),
                        Span::styled(details.stats.deaths.to_string(), s.palette.bad),
                        Span::raw("   Asistencias "),
                        Span::styled(details.stats.assists.to_string(), s.palette.focus),
                    ]));
                    s.row(Line::from(vec![
                        Span::raw(format!(
                            "K/D {}   KDA {}   HS ",
                            kd_text(details.stats.kills, details.stats.deaths),
                            kd_text(
                                details.stats.kills.saturating_add(details.stats.assists),
                                details.stats.deaths
                            )
                        )),
                        Span::styled(match_hs_text(&details.stats), s.palette.good),
                    ]));
                    s.text(format!(
                        "ACS {}   ADR {}   Rondas {}",
                        average_text(details.stats.combat_score, details.rounds_played, 0),
                        average_text(details.stats.damage, details.rounds_played, 1),
                        if details.rounds_played == 0 {
                            "—".into()
                        } else {
                            details.rounds_played.to_string()
                        }
                    ));
                    s.text(format!(
                        "Puntos totales {}   Daño total {}",
                        optional_number(details.stats.combat_score),
                        optional_number(details.stats.damage)
                    ));
                } else {
                    s.section("DETALLE NO DISPONIBLE", s.palette.pending);
                    s.text(format!(
                        "{} · {}\nNo se pudo obtener el detalle de esta partida.",
                        title_text(&item.entry.queue),
                        relative_time(item.entry.started_at_ms)
                    ));
                }
                s.anchor = Some(s.lines.len().saturating_sub(1));
            }
            s.section("RESUMEN", s.palette.focus);
            if let Some(summary) = history_summary(app) {
                s.text(summary);
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
    s.setting(
        0,
        Line::from(vec![
            Span::styled(
                if settings.selected == 0 { "› " } else { "  " },
                s.palette.focus,
            ),
            Span::raw("Tema de la interfaz        "),
            Span::styled(
                format!("[ {} ]", settings.draft.theme.label()),
                s.palette.focus,
            ),
        ]),
        app,
    );
    s.row(Line::styled(
        "  Cambia la paleta; la vista previa es inmediata.",
        s.palette.dim,
    ));
    s.row(Line::from(vec![
        Span::raw("  Tipografía                 "),
        Span::styled("Cascadia Mono recomendada", s.palette.base),
    ]));
    s.row(Line::styled(
        "  La fuente pertenece al terminal y se cambia allí.",
        s.palette.dim,
    ));

    s.section("ACTUALIZACIÓN", s.palette.focus);
    s.setting(
        1,
        Line::from(vec![
            Span::styled(
                if settings.selected == 1 { "› " } else { "  " },
                s.palette.focus,
            ),
            Span::raw("Frecuencia                  "),
            Span::styled(
                format!("[ {} s ]", settings.draft.interval.as_secs()),
                s.palette.focus,
            ),
        ]),
        app,
    );
    s.row(Line::styled(
        "  Cada cuánto se comprueba el estado del cliente (1–60 s).",
        s.palette.dim,
    ));
    s.setting(
        2,
        Line::from(vec![
            Span::styled(
                if settings.selected == 2 { "› " } else { "  " },
                s.palette.focus,
            ),
            Span::raw("Registro de diagnóstico     "),
            Span::styled(
                if settings.draft.log_transitions {
                    "[ Activado ]"
                } else {
                    "[ Desactivado ]"
                },
                if settings.draft.log_transitions {
                    s.palette.good
                } else {
                    s.palette.dim
                },
            ),
        ]),
        app,
    );
    s.row(Line::styled(
        "  Guarda fases en un archivo local; nunca credenciales.",
        s.palette.dim,
    ));

    s.section("CAMBIOS", s.palette.focus);
    s.row(Line::styled(
        settings.status(),
        if settings.draft == settings.active {
            s.palette.good
        } else {
            s.palette.pending
        },
    ));
    s.row(Line::from(vec![
        Span::styled("[s] Guardar", s.palette.good),
        Span::raw("   "),
        Span::styled("[r] Descartar", s.palette.bad),
    ]));

    s.section("PRIVACIDAD", s.palette.focus);
    s.row(Line::from(vec![
        Span::raw("Solo lectura               "),
        Span::styled("● Activo", s.palette.good),
    ]));
    s.text("No controla VALORANT, no accede a memoria y respeta nombres ocultos.");
    s.row(Line::styled(
        "↑/↓ elegir · +/- cambiar · Espacio alternar",
        s.palette.dim,
    ));
    if app.demo.is_some() {
        s.text("DEMO: guardar solo cambia esta sesión.");
    }
}

fn history_summary(app: &App) -> Option<String> {
    let details = app
        .history
        .as_ref()?
        .iter()
        .filter_map(|item| item.details.as_ref())
        .collect::<Vec<_>>();
    if details.is_empty() {
        return None;
    }
    let wins = details
        .iter()
        .filter(|details| details.outcome == MatchOutcome::Win)
        .count();
    let losses = details
        .iter()
        .filter(|details| details.outcome == MatchOutcome::Loss)
        .count();
    let kills = details.iter().map(|details| details.stats.kills).sum();
    let deaths = details.iter().map(|details| details.stats.deaths).sum();
    let assists = details.iter().map(|details| details.stats.assists).sum();
    let hs = details
        .iter()
        .try_fold((0_u32, 0_u32, 0_u32), |(head, body, leg), details| {
            Some((
                head.saturating_add(details.stats.headshots?),
                body.saturating_add(details.stats.bodyshots?),
                leg.saturating_add(details.stats.legshots?),
            ))
        })
        .and_then(|(head, body, leg)| headshot_percent(head, body, leg))
        .map_or_else(|| "—".into(), |value| format!("{value:.1}%"));
    Some(format!(
        "{} {} · {} {} · {} {}\nK/D {} · KDA {} · HS {}\n{} K / {} D / {} A",
        details.len(),
        plural(details.len(), "partida", "partidas"),
        wins,
        plural(wins, "victoria", "victorias"),
        losses,
        plural(losses, "derrota", "derrotas"),
        kd_text(kills, deaths),
        kd_text(kills.saturating_add(assists), deaths),
        hs,
        kills,
        deaths,
        assists
    ))
}

fn headshot_percent(headshots: u32, bodyshots: u32, legshots: u32) -> Option<f64> {
    let hits = headshots.checked_add(bodyshots)?.checked_add(legshots)?;
    (hits > 0).then(|| f64::from(headshots) * 100.0 / f64::from(hits))
}

fn match_hs_text(stats: &crate::models::PlayerMatchStats) -> String {
    match (stats.headshots, stats.bodyshots, stats.legshots) {
        (Some(head), Some(body), Some(leg)) => headshot_percent(head, body, leg)
            .map_or_else(|| "—".into(), |value| format!("{value:.1}%")),
        _ => "—".into(),
    }
}

fn average_text(total: Option<u32>, rounds: u32, decimals: usize) -> String {
    match (total, rounds) {
        (Some(total), rounds) if rounds > 0 => {
            format!("{:.*}", decimals, f64::from(total) / f64::from(rounds))
        }
        _ => "—".into(),
    }
}

fn optional_number(value: Option<u32>) -> String {
    value.map_or_else(|| "—".into(), |value| value.to_string())
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn kd_text(kills: u32, deaths: u32) -> String {
    if deaths == 0 {
        return if kills == 0 {
            "—".into()
        } else {
            "∞".into()
        };
    }
    format!("{:.2}", f64::from(kills) / f64::from(deaths))
}

fn score_text(own: Option<u32>, opponent: Option<u32>) -> Option<String> {
    Some(format!("{}-{}", own?, opponent?))
}

fn outcome_short(outcome: MatchOutcome) -> &'static str {
    match outcome {
        MatchOutcome::Win => "V",
        MatchOutcome::Loss => "D",
        MatchOutcome::Draw => "E",
        MatchOutcome::Unknown => "—",
    }
}

fn outcome_text(outcome: MatchOutcome) -> &'static str {
    match outcome {
        MatchOutcome::Win => "VICTORIA",
        MatchOutcome::Loss => "DERROTA",
        MatchOutcome::Draw => "EMPATE",
        MatchOutcome::Unknown => "PARTIDA FINALIZADA",
    }
}

fn title_text(value: &str) -> String {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + characters.as_str()
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
        Some(GamePhase::Idle) => "No estás en una partida",
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
    if startup_pending(app) {
        render_startup(area, frame, palette, app);
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
            0 if app.has_match_context() => "Enter abrir partida",
            0 => "3 perfil · 4 historial",
            1 if app.demo.as_ref().is_some_and(|d| d.post) => "p partida · Esc volver",
            1 if app.has_selectable_roster() => "↑↓ · Enter detalle · g Tracker",
            3 => "↑↓/clic partida · Enter detalle · r",
            4 => "↑↓/clic · +/- · s guardar · r descartar",
            _ => "r actualizar · PgUp/PgDn",
        }
    };
    let controls = if app.demo.is_some() && app.selected_tab == 1 {
        "1–5 · t · p fase · Esc · q salir"
    } else {
        "1–5 · t · Esc · q salir"
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

fn startup_pending(app: &App) -> bool {
    if app.demo.is_some() || app.completed_match.is_some() || app.live_match.is_some() {
        return false;
    }
    match app.state.as_ref() {
        None => true,
        Some(state) => state.client_found && app.own_profile.is_none() && !app.profile_failed,
    }
}

fn render_startup(area: Rect, frame: &mut ratatui::Frame<'_>, palette: Palette, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(palette.border)
        .title(Line::styled(" VTRACKER ", palette.focus));
    frame.render_widget(block, area);
    let height = 7_u16.min(area.height.saturating_sub(2));
    let top = area.y + area.height.saturating_sub(height) / 2;
    let inner = Rect::new(area.x + 2, top, area.width.saturating_sub(4), height);
    let (status, detail) = if app.state.is_none() {
        (
            "Buscando Riot Client…",
            "Comprobando el estado de tu sesión",
        )
    } else {
        (
            "Cargando tu perfil y rango…",
            "El historial continuará en segundo plano",
        )
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                "INICIANDO VTRACKER",
                palette.focus.add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(status, palette.pending),
            Line::styled(detail, palette.dim),
            Line::raw(""),
            Line::styled("Solo lectura · no controla VALORANT", palette.dim),
            Line::styled("q para salir", palette.dim),
        ])
        .alignment(Alignment::Center),
        inner,
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
                        assert!(
                            text.contains("q salir"),
                            "{width}x{height} tab {tab}\n{text}"
                        );
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
        app.live_match = None;
        app.completed_match = Some(super::super::PostMatch {
            mode: crate::models::GameMode::Deathmatch,
            map: "Ascent".into(),
            agent: "Sova".into(),
            outcome: MatchOutcome::Win,
            stats: crate::models::PlayerMatchStats {
                kills: 2,
                deaths: 1,
                assists: 0,
                ..Default::default()
            },
            own_score: None,
            opponent_score: None,
            rounds: vec![],
        });
        let summary = snapshot(&mut app, 72, 24);
        assert!(summary.contains("VICTORIA") && summary.contains("2 / 1 / 0"));
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
        app.own_profile = Some(crate::providers::profile::OwnProfile {
            level: 356,
            xp: 4_500,
        });
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
                level: DataAvailability::Available(142),
                premade: DataAvailability::Available("Grupo A".into()),
                stats,
            }
        };
        let mut roster = RosterSnapshot::new(vec![
            player(
                RosterSide::Ally,
                1,
                true,
                DataAvailability::Available("Norte#LAS".into()),
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
        .unwrap();
        // Current Game puede entregar cero para el jugador autenticado. Mi
        // perfil es la fuente autoritativa que debe ganar en pantalla.
        roster.players[0].level = DataAvailability::Available(0);
        app.live_match = Some(LiveMatchContext {
            map: "Haven".into(),
            mode: "Bomb".into(),
            agent: Some("Omen".into()),
            roster: Some(roster),
        });

        let text = snapshot(&mut app, 100, 30);
        assert!(text.contains("Norte#LAS") && text.contains("Omen") && text.contains("Diamante 1"));
        assert!(text.contains("Jugador 1") && text.contains("Jett") && text.contains("Radiante"));
        assert!(!text.contains("Jugador oculto"));
        assert!(text.contains("K/D") && text.contains("HS%") && text.contains("KAST%"));
        assert!(text.contains("1.50") && text.contains("50.0%") && text.contains("VD"));
        assert!(text.contains("NIV.") && !text.contains("PREMADE"));
        assert!(
            text.contains("356") && text.matches('•').count() == 2,
            "{text}"
        );
        assert!(text.contains("TRK") && text.contains("[↗]"));
        assert!(!text.contains("puuid") && !text.contains("local-websocket"));

        app.detail = true;
        let detail = snapshot(&mut app, 100, 30);
        assert!(detail.contains("[g] Abrir perfil en Tracker.gg"));
        assert!(detail.contains("Nivel 356"));
        assert!(detail.contains("Premade Grupo A · 2 jugadores"));

        let context = app.live_match.clone().unwrap();
        app.update_state(StateInfo::new(
            GamePhase::AgentSelect,
            GameState::Idle,
            Confidence::High,
            "local-websocket",
            true,
            true,
        ));
        app.live_match = Some(context);
        app.detail = false;
        let pregame = snapshot(&mut app, 100, 30);
        assert!(pregame.contains("COMPAÑEROS · SELECCIÓN DE AGENTE"));
        assert!(!pregame.contains("TUS RONDAS") && !pregame.contains("ENEMIGOS"));
    }

    #[test]
    fn real_panel_profile_history_and_settings_show_player_facing_content() {
        let mut app = App::new(&Config::default());
        app.update_state(StateInfo::new(
            GamePhase::Idle,
            GameState::Idle,
            Confidence::High,
            "local-client",
            true,
            false,
        ));
        app.own_profile = Some(crate::providers::profile::OwnProfile {
            level: 142,
            xp: 4_500,
        });
        app.competitive = Some(crate::providers::profile::CompetitiveProfile {
            tier: 18,
            ranked_rating: 64,
            wins: 8,
            games: 14,
        });
        app.history = Some(vec![super::super::HistoryItem {
            entry: crate::providers::history::HistoryEntry {
                queue: "competitivo".into(),
                started_at_ms: u64::MAX,
            },
            details: Some(super::super::HistoryDetails {
                map: "Ascent".into(),
                agent: "Sova".into(),
                outcome: MatchOutcome::Win,
                rounds_played: 21,
                stats: crate::models::PlayerMatchStats {
                    kills: 20,
                    deaths: 10,
                    assists: 5,
                    combat_score: Some(4_200),
                    damage: Some(3_150),
                    headshots: Some(12),
                    bodyshots: Some(30),
                    legshots: Some(6),
                },
                own_score: Some(13),
                opponent_score: Some(8),
            }),
        }]);

        app.select_tab(0);
        let panel = snapshot(&mut app, 100, 30);
        assert!(panel.contains("Nivel 142") && panel.contains("1 partida"));
        assert!(panel.contains("Diamante 1") && panel.contains("RR ██████░░░░ 64 / 100"));
        assert!(panel.contains("No estás en una partida"));
        assert!(panel.contains("K/D 2.00") && panel.contains("HS 25.0%"));
        assert!(panel.contains("MI PERFIL") && !panel.contains("FUENTES"));

        app.select_tab(2);
        let profile = snapshot(&mut app, 100, 30);
        assert!(profile.contains("20 K / 10 D / 5 A"));
        assert!(!profile.contains("pendiente") && !profile.contains("CLI"));

        app.select_tab(3);
        app.detail = true;
        let history = snapshot(&mut app, 100, 30);
        for value in [
            "VICTORIA",
            "13-8",
            "Ascent",
            "Sova",
            "Kills 20",
            "HS 25.0%",
            "ACS 200",
            "ADR 150.0",
        ] {
            assert!(history.contains(value), "missing {value}\n{history}");
        }

        app.select_tab(1);
        app.completed_match = Some(super::super::PostMatch {
            mode: crate::models::GameMode::Competitive,
            map: "Ascent".into(),
            agent: "Sova".into(),
            outcome: MatchOutcome::Win,
            stats: crate::models::PlayerMatchStats {
                kills: 20,
                deaths: 10,
                assists: 5,
                combat_score: Some(4_200),
                ..Default::default()
            },
            own_score: Some(13),
            opponent_score: Some(8),
            rounds: vec![
                super::super::PostRound {
                    number: 1,
                    result: "eliminación".into(),
                    kills: 2,
                    deaths: 0,
                },
                super::super::PostRound {
                    number: 2,
                    result: "desactivada".into(),
                    kills: 0,
                    deaths: 1,
                },
            ],
        });
        let postmatch = snapshot(&mut app, 100, 30);
        assert!(postmatch.contains("TU RESULTADO") && postmatch.contains("TUS RONDAS"));

        app.select_tab(4);
        let settings = snapshot(&mut app, 100, 30);
        assert!(settings.contains("Frecuencia"));
        assert!(settings.contains("Registro de diagnóstico"));
        assert!(settings.contains("Cascadia Mono recomendada"));
        assert!(settings.contains("Solo lectura"));
        assert!(!settings.contains("Log cambios") && !settings.contains("Retratos opcionales"));
    }

    #[test]
    fn pristine_app_renders_startup_screen_before_the_first_observation() {
        let mut app = App::new(&Config::default());
        let startup = snapshot(&mut app, 72, 24);

        assert!(startup.contains("INICIANDO VTRACKER"));
        assert!(startup.contains("Buscando Riot Client"));
        assert!(startup.contains("Comprobando el estado de tu sesión"));
        assert!(startup.contains("Solo lectura"));
        assert!(!startup.contains("1 Resumen") && !startup.contains("ESTADO DE PARTIDA"));

        app.update_state(StateInfo::new(
            GamePhase::GameOpen,
            GameState::GameOpen,
            Confidence::High,
            "local-client",
            true,
            true,
        ));
        let profile_loading = snapshot(&mut app, 72, 24);
        assert!(profile_loading.contains("Cargando tu perfil y rango"));
        assert!(profile_loading.contains("historial continuará en segundo plano"));
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
