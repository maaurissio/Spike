//! Interfaz terminal interactiva, sin I/O remoto durante el renderizado.

use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
};

use crate::{
    config::Config,
    providers::{
        HistorySource, LiveMatchSource, LocalClientSource, PlayerProfileSource,
        ProcessGameStateSource, StateInfo,
        history::HistoryEntry,
        live_match::LiveMatchContext,
        match_detail::{CompletedMatch, MatchDetailSource},
        profile::{CompetitiveProfile, CompetitiveUpdate, OwnProfile},
        resolve_with_fallback,
    },
};

const TABS: [&str; 5] = ["Panel", "Partida", "Perfil", "Historial", "Ajustes"];
const INPUT_TIMEOUT: Duration = Duration::from_millis(100);

struct App {
    selected_tab: usize,
    state: Option<StateInfo>,
    live_match: Option<LiveMatchContext>,
    completed_match: Option<CompletedMatch>,
    own_profile: Option<OwnProfile>,
    competitive: Option<CompetitiveProfile>,
    competitive_updates: Vec<CompetitiveUpdate>,
    history: Option<Vec<HistoryEntry>>,
    history_failed: bool,
    interval_secs: u64,
    log_transitions: bool,
    refresh_failed: bool,
    dirty: bool,
    should_quit: bool,
}

impl App {
    fn new(config: &Config) -> Self {
        Self {
            selected_tab: 0,
            state: None,
            live_match: None,
            completed_match: None,
            own_profile: None,
            competitive: None,
            competitive_updates: Vec::new(),
            history: None,
            history_failed: false,
            interval_secs: config.interval.as_secs(),
            log_transitions: config.log_transitions,
            refresh_failed: false,
            dirty: true,
            should_quit: false,
        }
    }

    fn select_next(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % TABS.len();
        self.dirty = true;
    }

    fn select_previous(&mut self) {
        self.selected_tab = (self.selected_tab + TABS.len() - 1) % TABS.len();
        self.dirty = true;
    }

    fn update_state(&mut self, state: StateInfo) -> bool {
        let phase_changed = self
            .state
            .as_ref()
            .is_none_or(|previous| previous.phase != state.phase);
        let changed = self.state.as_ref().is_none_or(|previous| {
            previous.phase != state.phase
                || previous.client_found != state.client_found
                || previous.game_found != state.game_found
        });
        self.state = Some(state);
        self.refresh_failed = false;
        self.dirty |= changed;
        phase_changed
    }

    fn mark_refresh_failed(&mut self) {
        if !self.refresh_failed {
            self.refresh_failed = true;
            self.dirty = true;
        }
    }
}

/// Abre el dashboard. El estado se consulta fuera de `draw`, a un intervalo
/// acotado, y el render solo ocurre cuando cambian datos, pestaña o tamaño.
pub(crate) fn run(config: Config) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_loop(&mut terminal, config);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
) -> io::Result<()> {
    let refresh_interval = config.interval;
    let local = LocalClientSource::new();
    let fallback = ProcessGameStateSource::new();
    let history_source = HistorySource::new();
    let live_match_source = LiveMatchSource::new();
    let match_detail_source = MatchDetailSource::new();
    let profile_source = PlayerProfileSource::new();
    local.start_event_listener();
    let mut app = App::new(&config);
    let mut last_refresh = Instant::now()
        .checked_sub(refresh_interval)
        .unwrap_or_else(Instant::now);

    while !app.should_quit {
        if last_refresh.elapsed() >= refresh_interval {
            match resolve_with_fallback(&local, &fallback) {
                Ok(state) => {
                    if app.update_state(state) {
                        refresh_player_context(
                            &mut app,
                            &local,
                            &live_match_source,
                            &match_detail_source,
                            &profile_source,
                        );
                    }
                }
                Err(_) => app.mark_refresh_failed(),
            }
            last_refresh = Instant::now();
        }

        if app.selected_tab == 3 && app.history.is_none() && !app.history_failed {
            refresh_history(&mut app, &local, &history_source);
        }

        if app.dirty {
            terminal.draw(|frame| render(frame.area(), frame, &app))?;
            app.dirty = false;
        }

        if event::poll(INPUT_TIMEOUT)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.select_next(),
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => app.select_previous(),
                    KeyCode::Char('r') => {
                        if app.selected_tab == 3 {
                            refresh_history(&mut app, &local, &history_source);
                        } else {
                            refresh_player_context(
                                &mut app,
                                &local,
                                &live_match_source,
                                &match_detail_source,
                                &profile_source,
                            );
                        }
                    }
                    _ => {}
                },
                Event::Resize(_, _) => app.dirty = true,
                _ => {}
            }
        }
    }
    Ok(())
}

/// Historial acotado del jugador autenticado. No se guardan ni muestran IDs de
/// partida y el usuario controla los refrescos posteriores con `r`.
fn refresh_history(app: &mut App, local: &LocalClientSource, source: &HistorySource) {
    match local
        .history_request(10)
        .and_then(|request| source.fetch_own(&request))
    {
        Ok(entries) => {
            app.history = Some(entries);
            app.history_failed = false;
        }
        Err(_) => {
            app.history = None;
            app.history_failed = true;
        }
    }
    app.dirty = true;
}

/// Las consultas remotas son puntuales: solo al entrar en una fase que puede
/// aportar datos nuevos. Nunca se ejecutan desde el bucle de renderizado.
fn refresh_player_context(
    app: &mut App,
    local: &LocalClientSource,
    live_match_source: &LiveMatchSource,
    match_detail_source: &MatchDetailSource,
    profile_source: &PlayerProfileSource,
) {
    let phase = app.state.as_ref().map(|state| state.phase);
    match phase {
        Some(crate::providers::capabilities::GamePhase::InMatch) => {
            app.live_match = local
                .live_match_request()
                .and_then(|request| live_match_source.fetch(&request))
                .ok();
            app.completed_match = None;
            app.own_profile = None;
            app.competitive = None;
            app.competitive_updates.clear();
        }
        Some(crate::providers::capabilities::GamePhase::Idle) => {
            let profile_data = local.profile_request().and_then(|request| {
                profile_source.fetch_own(&request).map(|profile| {
                    let competitive = profile_source
                        .fetch_own_competitive(&request)
                        .ok()
                        .flatten();
                    let updates = profile_source
                        .fetch_own_competitive_updates(&request, 5)
                        .unwrap_or_default();
                    (profile, competitive, updates)
                })
            });
            match profile_data {
                Ok((profile, competitive, updates)) => {
                    app.own_profile = Some(profile);
                    app.competitive = competitive;
                    app.competitive_updates = updates;
                }
                Err(_) => {
                    app.own_profile = None;
                    app.competitive = None;
                    app.competitive_updates.clear();
                }
            }
            app.live_match = None;
            app.completed_match = None;
        }
        Some(crate::providers::capabilities::GamePhase::PostMatch) => {
            app.completed_match = local
                .match_detail_request()
                .and_then(|request| match_detail_source.fetch_completed(&request))
                .ok();
            app.live_match = None;
            app.own_profile = None;
            app.competitive = None;
            app.competitive_updates.clear();
        }
        _ => {
            app.live_match = None;
            app.completed_match = None;
            app.own_profile = None;
            app.competitive = None;
            app.competitive_updates.clear();
        }
    }
    app.dirty = true;
}

fn render(area: Rect, frame: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let titles = TABS.iter().map(|tab| Line::from(*tab)).collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .select(app.selected_tab)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title(" VTRACKER "),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" · "),
        chunks[0],
    );

    frame.render_widget(main_content(app), chunks[1]);
    frame.render_widget(
        Paragraph::new("←/→ cambiar vista   ·   r actualizar   ·   q salir")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
}

fn main_content(app: &App) -> Paragraph<'static> {
    Paragraph::new(content_for(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(TABS[app.selected_tab]),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left)
}

fn content_for(app: &App) -> String {
    let phase = app
        .state
        .as_ref()
        .map(|state| state.phase.label())
        .unwrap_or("Comprobando cliente…");
    let body = match app.selected_tab {
        0 => dashboard_content(app, phase),
        1 => match app.completed_match.as_ref() {
            Some(completed) => completed_match_content(completed),
            None => match app.live_match.as_ref() {
                Some(match_context) => format!(
                "Partida en curso\n\nModo     {}\nMapa     {}\nAgente   {}\n\nSolo se muestra tu contexto; el tracker no consulta ni expone roster, rangos o perfiles de otras personas.",
                match_context.mode,
                match_context.map,
                match_context.agent.as_deref().unwrap_or("no disponible"),
            ),
                None => "Partida\n\nNo hay detalles de partida todavía.\n\nEsta vista se completa al recibir una fase de partida confirmada.".into(),
            },
        },
        2 => match app.own_profile.as_ref() {
            Some(profile) => profile_content(app, profile),
            None => "Perfil\n\nTu perfil se consulta al quedar disponible el cliente.\n\nTambién puedes usar `vtracker player` cuando quieras un resumen puntual.".into(),
        },
        3 => history_content(app),
        _ => format!(
            "Ajustes\n\nIntervalo de actualización   {} s\nRegistrar transiciones        {}\n\nUsa `vtracker config show`, `validate` o `edit` para cambiar la configuración. Los secretos no se muestran ni se guardan aquí.",
            app.interval_secs,
            if app.log_transitions { "sí" } else { "no" },
        ),
    };
    let footer = if app.refresh_failed {
        "\n\nNo se pudo actualizar ahora; se reintentará automáticamente."
    } else {
        ""
    };
    format!("{body}{footer}")
}

fn completed_match_content(completed: &CompletedMatch) -> String {
    if let Some(rounds) = completed.rounds.as_ref() {
        let mut content = format!(
            "Última partida · {} · {} rondas\n\nRonda  Resultado        K  D\n",
            rounds.mode.label(),
            rounds.rounds.len(),
        );
        for round in &rounds.rounds {
            if let Some(player) = round
                .players
                .iter()
                .find(|player| player.puuid == completed.own_puuid)
            {
                content.push_str(&format!(
                    "{:>2}     {:<16} {:>1}  {:>1}\n",
                    round.round_num,
                    round.round_result.label(),
                    player.kills,
                    player.deaths,
                ));
            }
        }
        return content;
    }
    if let Some(summary) = completed.summary.as_ref() {
        return format!(
            "Última partida · {}\n\nK  D  A  Puntos\n{}  {}  {}  {}",
            summary.mode.label(),
            summary.stats.kills,
            summary.stats.deaths,
            summary.stats.assists,
            summary.stats.combat_score.unwrap_or(0),
        );
    }
    "Última partida\n\nEl resumen propio aún no está disponible.".into()
}

fn profile_content(app: &App, profile: &OwnProfile) -> String {
    let mut content = format!(
        "Perfil propio\n\nNivel de cuenta   {}\nExperiencia       {} XP",
        profile.level, profile.xp,
    );
    if let Some(competitive) = app.competitive.as_ref() {
        content.push_str(&format!(
            "\nRango              {} · {} RR\nCompetitivo        {}/{} victorias",
            crate::ui::competitive_tier_label(competitive.tier),
            competitive.ranked_rating,
            competitive.wins,
            competitive.games,
        ));
    }
    if !app.competitive_updates.is_empty() {
        content.push_str("\nCambios RR         ");
        for (index, update) in app.competitive_updates.iter().enumerate() {
            if index > 0 {
                content.push_str(" · ");
            }
            content.push_str(&format!("{:+}", update.rr_earned));
            if update.performance_bonus > 0 {
                content.push_str(&format!(" (+{} bono)", update.performance_bonus));
            }
        }
    }
    content
}

fn dashboard_content(app: &App, phase: &str) -> String {
    let mut content = format!("Estado de VALORANT\n\n{phase}");
    if let Some(match_context) = app.live_match.as_ref() {
        content.push_str(&format!(
            "\n\nPartida actual\n{} · {} · {}",
            match_context.mode,
            match_context.map,
            match_context.agent.as_deref().unwrap_or("sin agente"),
        ));
    } else if let Some(profile) = app.own_profile.as_ref() {
        content.push_str(&format!(
            "\n\nPerfil propio\nNivel {} · {} XP",
            profile.level, profile.xp,
        ));
    } else {
        content.push_str("\n\nEl tracker observa el cliente de forma local y en modo lectura.");
    }
    content
}

fn history_content(app: &App) -> String {
    if app.history_failed {
        return "Historial\n\nNo se pudo cargar tu historial ahora. Presiona `r` para reintentar."
            .into();
    }
    let Some(entries) = app.history.as_ref() else {
        return "Historial\n\nCargando tus partidas recientes…".into();
    };
    if entries.is_empty() {
        return "Historial\n\nNo hay partidas recientes disponibles.".into();
    }
    let mut output = String::from("Historial propio\n\nModo                 Cuándo\n");
    for entry in entries {
        output.push_str(&format!(
            "{:<20} {}\n",
            entry.queue,
            relative_time(entry.started_at_ms),
        ));
    }
    output.push_str("\nPresiona `r` para actualizar.");
    output
}

fn relative_time(started_at_ms: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    match now.saturating_sub(started_at_ms) / 1_000 {
        0..=59 => "ahora".into(),
        60..=3_599 => format!("hace {} min", now.saturating_sub(started_at_ms) / 60_000),
        3_600..=86_399 => format!("hace {} h", now.saturating_sub(started_at_ms) / 3_600_000),
        _ => format!("hace {} d", now.saturating_sub(started_at_ms) / 86_400_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::GameState,
        providers::capabilities::{Confidence, GamePhase},
    };

    #[test]
    fn navigation_wraps_between_available_views() {
        let mut app = App::new(&Config::default());
        app.select_previous();
        assert_eq!(app.selected_tab, TABS.len() - 1);
        app.select_next();
        assert_eq!(app.selected_tab, 0);
    }

    #[test]
    fn dashboard_uses_player_facing_status_only() {
        let mut app = App::new(&Config::default());
        app.update_state(StateInfo::new(
            GamePhase::InMatch,
            GameState::GameOpen,
            Confidence::High,
            "local-websocket",
            true,
            true,
        ));
        let text = content_for(&app);
        assert!(text.contains("En partida"));
        assert!(!text.contains("local-websocket"));
        assert!(!text.contains("Confianza"));
    }

    #[test]
    fn history_view_never_exposes_match_identifiers() {
        let mut app = App::new(&Config::default());
        app.history = Some(vec![HistoryEntry {
            queue: "competitivo".into(),
            started_at_ms: 0,
        }]);

        let text = history_content(&app);
        assert!(text.contains("competitivo"));
        assert!(!text.contains("match"));
    }
}
