//! Interfaz terminal interactiva, sin I/O remoto durante el renderizado.

mod demo;
mod metrics;
mod settings;
mod theme;
mod view;
mod worker;

use std::{
    collections::VecDeque,
    env, io,
    process::Command as ProcessCommand,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::MoveTo,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use serde::{Deserialize, Serialize};

use crate::{
    config::Config,
    models::{
        GameMode, MatchOutcome, PlayerMatchStats,
        roster::{DataAvailability, RosterPlayer, RosterSide},
    },
    providers::{
        StateInfo,
        capabilities::GamePhase,
        history::HistoryEntry,
        live_match::LiveMatchContext,
        match_detail::{CompletedMatch, CompletedPlayerSide, CompletedRosterPlayer},
        profile::{CompetitiveProfile, CompetitiveUpdate, OwnProfile},
    },
};
use settings::Settings;
use view::render;
use worker::{Context, Reply, Request, Worker};

const TABS: [&str; 6] = [
    "Resumen",
    "Partida",
    "Mi perfil",
    "Historial",
    "Ajustes",
    "Logs",
];
const INPUT_TIMEOUT: Duration = Duration::from_millis(100);
const SPLASH_DURATION: Duration = Duration::from_secs(3);
const PARTY_REFRESH_INTERVAL: Duration = Duration::from_secs(4);
const MAX_PARTY_REFRESH_ATTEMPTS: u8 = 8;

/// Las cinco vistas persistentes. `Partida` se muestra aparte solo si hay contexto activo.
const BASE_TABS: [usize; 5] = [0, 2, 3, 4, 5];

fn tab_indices(width: u16, row: usize, match_visible: bool) -> &'static [usize] {
    const WIDE: [usize; 5] = [0, 2, 3, 4, 5];
    const COMPACT_TOP: [usize; 2] = [0, 2];
    const COMPACT_MIDDLE: [usize; 2] = [3, 4];
    const COMPACT_BOTTOM: [usize; 1] = [5];
    const COMPACT_ACTIVE_TOP: [usize; 1] = [0];
    const COMPACT_ACTIVE_MIDDLE: [usize; 2] = [2, 3];
    const COMPACT_ACTIVE_BOTTOM: [usize; 2] = [4, 5];
    const MEDIUM_TOP: [usize; 3] = [0, 2, 3];
    const MEDIUM_BOTTOM: [usize; 2] = [4, 5];
    const MEDIUM_ACTIVE_TOP: [usize; 2] = [0, 2];
    const MEDIUM_ACTIVE_BOTTOM: [usize; 3] = [3, 4, 5];
    if width >= 90 {
        &WIDE
    } else if width >= 58 {
        if row == 0 {
            if match_visible {
                &MEDIUM_ACTIVE_TOP
            } else {
                &MEDIUM_TOP
            }
        } else if match_visible {
            &MEDIUM_ACTIVE_BOTTOM
        } else {
            &MEDIUM_BOTTOM
        }
    } else if row == 0 {
        if match_visible {
            &COMPACT_ACTIVE_TOP
        } else {
            &COMPACT_TOP
        }
    } else if row == 1 {
        if match_visible {
            &COMPACT_ACTIVE_MIDDLE
        } else {
            &COMPACT_MIDDLE
        }
    } else if match_visible {
        &COMPACT_ACTIVE_BOTTOM
    } else {
        &COMPACT_BOTTOM
    }
}

fn tab_rows(width: u16) -> u16 {
    if width >= 90 {
        1
    } else if width >= 58 {
        2
    } else {
        3
    }
}

fn tab_text(index: usize, compact: bool) -> String {
    let label = if compact {
        match index {
            2 => "Perfil",
            3 => "Hist.",
            4 => "Ajust.",
            5 => "Logs",
            _ => TABS[index],
        }
    } else {
        TABS[index]
    };
    let number = match index {
        0 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        _ => 0,
    };
    if compact {
        format!(" {number}: {label} ")
    } else {
        format!("  {number}: {label}  ")
    }
}

fn match_tab_text(compact: bool) -> &'static str {
    if compact { " Partida " } else { "  Partida  " }
}

fn match_tab_x(width: u16, compact: bool) -> u16 {
    width.saturating_sub(match_tab_text(compact).chars().count() as u16 + 1)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HistoryDetails {
    map: String,
    agent: String,
    outcome: MatchOutcome,
    rounds_played: u32,
    stats: PlayerMatchStats,
    own_score: Option<u32>,
    opponent_score: Option<u32>,
    roster: Vec<CompletedRosterPlayer>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HistoryItem {
    entry: HistoryEntry,
    details: Option<HistoryDetails>,
    rr_change: Option<i32>,
    rr_after: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CachedHistory {
    schema: u8,
    saved_at_ms: u64,
    items: Vec<HistoryItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PostRound {
    number: u32,
    result: String,
    kills: u8,
    deaths: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PostMatch {
    mode: GameMode,
    map: String,
    agent: String,
    outcome: MatchOutcome,
    stats: PlayerMatchStats,
    own_score: Option<u32>,
    opponent_score: Option<u32>,
    rounds: Vec<PostRound>,
}

impl PostMatch {
    fn from_completed(completed: CompletedMatch) -> Self {
        let mode = completed
            .rounds
            .as_ref()
            .map(|rounds| rounds.mode)
            .or_else(|| completed.summary.as_ref().map(|summary| summary.mode))
            .unwrap_or_default();
        let rounds = completed
            .rounds
            .as_ref()
            .into_iter()
            .flat_map(|rounds| &rounds.rounds)
            .filter_map(|round| {
                round
                    .players
                    .iter()
                    .find(|player| player.puuid == completed.own_puuid)
                    .map(|player| PostRound {
                        number: round.round_num,
                        result: round.round_result.label().into(),
                        kills: player.kills,
                        deaths: player.deaths,
                    })
            })
            .collect();
        let totals = completed.totals;
        Self {
            mode,
            map: totals.map,
            agent: totals.agent,
            outcome: totals.stats.outcome,
            stats: totals.stats.stats,
            own_score: totals.own_score,
            opponent_score: totals.opponent_score,
            rounds,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tabs,
    Content,
}

#[derive(Clone, Copy)]
enum LogLevel {
    Info,
    Success,
    Warning,
}

struct LogEntry {
    at: Duration,
    level: LogLevel,
    message: String,
}

/// Actividad de red propia durante esta ejecución. Cuenta sincronizaciones de
/// alto nivel con Riot, no bytes ni tráfico de otros procesos del equipo.
#[derive(Default)]
struct NetworkUsage {
    riot_syncs: u32,
    failed_syncs: u32,
    last_sync: Option<NetworkSync>,
}

#[derive(Clone, Copy)]
struct NetworkSync {
    at: Duration,
    succeeded: bool,
}

struct App {
    splash_started: Instant,
    splash_complete: bool,
    demo: Option<demo::Demo>,
    focus: Focus,
    player_index: usize,
    history_index: usize,
    detail: bool,
    tracker_notice: bool,
    tracker_open_failed: bool,
    round_page: usize,
    follow_selection: bool,
    selected_tab: usize,
    state: Option<StateInfo>,
    live_match: Option<LiveMatchContext>,
    completed_match: Option<PostMatch>,
    own_profile: Option<OwnProfile>,
    competitive: Option<CompetitiveProfile>,
    competitive_updates: Vec<CompetitiveUpdate>,
    history: Option<Vec<HistoryItem>>,
    history_cached_at_ms: Option<u64>,
    postmatch_history_ready: bool,
    postmatch_history_refresh_pending: bool,
    history_failed: bool,
    history_requested: bool,
    profile_pending: bool,
    profile_requested: bool,
    profile_failed: bool,
    settings: Settings,
    observation_pending: bool,
    context_pending: bool,
    context_requested: bool,
    context_failed: bool,
    context_progress: u16,
    context_progress_label: &'static str,
    party_pending: bool,
    party_attempts: u8,
    party_retry_at: Option<Instant>,
    history_pending: bool,
    generation: u64,
    epoch: u64,
    log_failed: bool,
    scroll: u16,
    refresh_failed: bool,
    dirty: bool,
    should_quit: bool,
    quit_after_save: bool,
    restart_requested: bool,
    metrics: metrics::ProcessMetrics,
    network: NetworkUsage,
    logs: VecDeque<LogEntry>,
    custom_palette: Option<theme::EditablePalette>,
}

impl App {
    fn new(config: &Config) -> Self {
        let mut app = Self {
            splash_started: Instant::now(),
            splash_complete: false,
            demo: None,
            focus: Focus::Content,
            player_index: 0,
            history_index: 0,
            detail: false,
            tracker_notice: false,
            tracker_open_failed: false,
            round_page: 0,
            follow_selection: true,
            selected_tab: 0,
            state: None,
            live_match: None,
            completed_match: None,
            own_profile: None,
            competitive: None,
            competitive_updates: Vec::new(),
            history: None,
            history_cached_at_ms: None,
            postmatch_history_ready: false,
            postmatch_history_refresh_pending: false,
            history_failed: false,
            history_requested: false,
            profile_pending: false,
            profile_requested: false,
            profile_failed: false,
            settings: Settings::new(config),
            observation_pending: false,
            context_pending: false,
            context_requested: false,
            context_failed: false,
            context_progress: 0,
            context_progress_label: "Esperando contexto de partida",
            party_pending: false,
            party_attempts: 0,
            party_retry_at: None,
            history_pending: false,
            generation: 0,
            epoch: 0,
            log_failed: false,
            scroll: 0,
            refresh_failed: false,
            dirty: true,
            should_quit: false,
            quit_after_save: false,
            restart_requested: false,
            metrics: metrics::ProcessMetrics::new(),
            network: NetworkUsage::default(),
            logs: VecDeque::new(),
            custom_palette: None,
        };
        app.push_log(LogLevel::Info, "Interfaz iniciada");
        app
    }

    fn select_next(&mut self) {
        let tabs = self.navigation_tabs();
        let index = tabs
            .iter()
            .position(|&tab| tab == self.selected_tab)
            .unwrap_or(0);
        self.select_tab(tabs[(index + 1) % tabs.len()]);
    }

    fn tick(&mut self) {
        if !self.splash_complete {
            if self.splash_started.elapsed() >= SPLASH_DURATION {
                self.splash_complete = true;
            }
            // El logo tiene un degradado animado discreto y debe redibujarse;
            // al cumplir tres segundos este mismo tick presenta la vista real.
            self.dirty = true;
        }
        self.dirty |= self.metrics.tick();
    }

    fn push_log(&mut self, level: LogLevel, message: impl Into<String>) {
        const LOG_LIMIT: usize = 100;
        self.logs.push_back(LogEntry {
            at: self.metrics.uptime(),
            level,
            message: message.into(),
        });
        while self.logs.len() > LOG_LIMIT {
            self.logs.pop_front();
        }
        self.dirty = true;
    }

    fn reload_palette(&mut self) {
        match theme::load_or_create_palette() {
            Ok(palette) => {
                self.custom_palette = Some(palette);
                self.push_log(LogLevel::Success, "Paleta recargada desde palette.toml");
            }
            Err(error) => {
                self.push_log(LogLevel::Warning, format!("Paleta inválida: {error}"));
            }
        }
        self.dirty = true;
    }

    fn open_palette_editor(&mut self) {
        match open_palette_folder() {
            Ok(()) => self.push_log(LogLevel::Success, "Carpeta de palette.toml abierta"),
            Err(error) => self.push_log(
                LogLevel::Warning,
                format!("No se pudo abrir la carpeta de la paleta: {error}"),
            ),
        }
    }

    fn record_riot_sync(&mut self, label: &'static str, succeeded: bool) {
        self.network.riot_syncs = self.network.riot_syncs.saturating_add(1);
        if !succeeded {
            self.network.failed_syncs = self.network.failed_syncs.saturating_add(1);
        }
        self.network.last_sync = Some(NetworkSync {
            at: self.metrics.uptime(),
            succeeded,
        });
        self.push_log(
            if succeeded {
                LogLevel::Success
            } else {
                LogLevel::Warning
            },
            if succeeded {
                format!("Red Riot: {label} sincronizado")
            } else {
                format!("Red Riot: no se pudo sincronizar {label}")
            },
        );
    }

    fn select_previous(&mut self) {
        let tabs = self.navigation_tabs();
        let index = tabs
            .iter()
            .position(|&tab| tab == self.selected_tab)
            .unwrap_or(0);
        self.select_tab(tabs[(index + tabs.len() - 1) % tabs.len()]);
    }

    fn select_tab(&mut self, tab: usize) {
        self.selected_tab = if tab == 1 && !self.match_tab_visible() {
            0
        } else {
            tab
        };
        self.scroll = 0;
        self.detail = false;
        self.tracker_notice = false;
        self.tracker_open_failed = false;
        self.follow_selection = true;
        self.dirty = true;
    }

    fn has_match_context(&self) -> bool {
        self.match_tab_visible()
    }

    fn match_tab_visible(&self) -> bool {
        self.demo.is_some()
            || self.state.as_ref().is_some_and(|state| {
                matches!(
                    state.phase,
                    GamePhase::PreGame
                        | GamePhase::AgentSelect
                        | GamePhase::InMatch
                        | GamePhase::PostMatch
                )
            })
    }

    fn navigation_tabs(&self) -> Vec<usize> {
        let mut tabs = BASE_TABS.to_vec();
        if self.match_tab_visible() {
            tabs.push(1);
        }
        tabs
    }

    fn selected_live_player(&self) -> Option<&RosterPlayer> {
        self.live_match
            .as_ref()?
            .roster
            .as_ref()?
            .players
            .get(self.player_index)
    }

    fn selected_history_player(&self) -> Option<&CompletedRosterPlayer> {
        self.history
            .as_ref()?
            .get(self.history_index)?
            .details
            .as_ref()?
            .roster
            .get(self.player_index)
    }

    fn selected_postmatch_player(&self) -> Option<&CompletedRosterPlayer> {
        if !self.postmatch_history_ready {
            return None;
        }
        self.history
            .as_ref()?
            .first()?
            .details
            .as_ref()?
            .roster
            .get(self.player_index)
    }

    fn has_selectable_roster(&self) -> bool {
        self.demo.is_some()
            || self.selected_live_player().is_some()
            || self.selected_postmatch_player().is_some()
    }

    fn update_state(&mut self, state: StateInfo) -> bool {
        let reconnected = self
            .state
            .as_ref()
            .is_some_and(|previous| !previous.client_found && state.client_found);
        let phase_changed = self
            .state
            .as_ref()
            .is_none_or(|previous| previous.phase != state.phase);
        let changed = self.state.as_ref().is_none_or(|previous| {
            previous.phase != state.phase
                || previous.client_found != state.client_found
                || previous.game_found != state.game_found
        });
        if phase_changed {
            self.push_log(LogLevel::Info, format!("Estado: {}", state.phase.label()));
            self.scroll = 0;
            self.detail = false;
            self.follow_selection = true;
            self.generation = self.generation.wrapping_add(1);
            self.context_requested = false;
            self.context_failed = false;
            self.party_pending = false;
            self.party_attempts = 0;
            self.party_retry_at = None;
            self.postmatch_history_ready = false;
            self.postmatch_history_refresh_pending = false;
            self.live_match = None;
            // El último resumen permanece disponible tras volver al menú.
            if matches!(state.phase, GamePhase::InMatch | GamePhase::ClientClosed) {
                self.completed_match = None;
            }
            if state.phase == GamePhase::PostMatch {
                self.history_requested = false;
                self.history_failed = false;
                self.history_index = 0;
                self.player_index = 0;
            }
            if state.phase == GamePhase::ClientClosed {
                self.epoch = self.epoch.wrapping_add(1);
                self.own_profile = None;
                self.competitive = None;
                self.competitive_updates.clear();
                self.profile_pending = false;
                self.profile_requested = false;
                self.profile_failed = false;
                // El historial seguro persiste para seguir disponible sin VALORANT.
                self.history_failed = false;
                self.history_requested = false;
            }
        }
        if reconnected {
            self.history_failed = false;
            self.history_requested = false;
        }
        self.dirty |= changed || self.refresh_failed;
        self.state = Some(state);
        if self.selected_tab == 1 && !self.match_tab_visible() {
            self.select_tab(0);
        }
        self.refresh_failed = false;
        phase_changed
    }

    fn mark_refresh_failed(&mut self) {
        if !self.refresh_failed {
            self.refresh_failed = true;
            self.push_log(LogLevel::Warning, "Conexión interrumpida; reintentando");
            self.dirty = true;
        }
    }

    fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::Observed { state, log_failed } => {
                self.observation_pending = false;
                self.dirty |= self.log_failed != log_failed;
                if log_failed && !self.log_failed {
                    self.push_log(
                        LogLevel::Warning,
                        "No se pudo escribir el registro en disco",
                    );
                }
                self.log_failed = log_failed;
                match state {
                    Ok(state) => {
                        self.update_state(state);
                    }
                    Err(()) => self.mark_refresh_failed(),
                }
                return;
            }
            Reply::Context { generation, data } => {
                self.context_pending = false;
                self.record_riot_sync("contexto de partida", data.is_ok());
                if generation != self.generation {
                    self.dirty = true;
                    return;
                }
                self.context_failed = data.is_err();
                self.context_progress = if data.is_ok() { 100 } else { 0 };
                match data {
                    Ok(Context::Live(context)) => {
                        self.player_index = context
                            .roster
                            .as_ref()
                            .and_then(|roster| {
                                roster.players.iter().position(|player| player.is_self)
                            })
                            .unwrap_or(0);
                        self.live_match = Some(context);
                        self.party_pending = false;
                        self.party_attempts = 0;
                        self.party_retry_at = Some(Instant::now() + PARTY_REFRESH_INTERVAL);
                        self.push_log(LogLevel::Success, "Contexto de partida actualizado");
                    }
                    Ok(Context::Completed(summary)) => {
                        self.completed_match = Some(summary);
                        self.push_log(LogLevel::Success, "Resumen postpartida actualizado");
                    }
                    Err(()) => self.push_log(
                        LogLevel::Warning,
                        "No se pudo actualizar el contexto de partida",
                    ),
                }
            }
            Reply::Parties { generation, data } => {
                self.party_pending = false;
                if generation != self.generation {
                    self.dirty = true;
                    return;
                }
                let mut complete = false;
                if let Ok(update) = data {
                    complete = update.complete;
                    if let Some(roster) = self
                        .live_match
                        .as_mut()
                        .and_then(|context| context.roster.as_mut())
                        && roster.players.len() == update.premades.len()
                    {
                        for (player, premade) in roster.players.iter_mut().zip(update.premades) {
                            let inferred_group = matches!(
                                &player.premade,
                                DataAvailability::Available(label) if label.starts_with("Grupo ")
                            );
                            let refreshed_group = matches!(
                                &premade,
                                DataAvailability::Available(label) if label.starts_with("Grupo ")
                            );
                            if refreshed_group || !inferred_group {
                                player.premade = premade;
                            }
                        }
                    }
                }
                self.party_retry_at =
                    if complete || self.party_attempts >= MAX_PARTY_REFRESH_ATTEMPTS {
                        None
                    } else {
                        Some(Instant::now() + PARTY_REFRESH_INTERVAL)
                    };
            }
            Reply::ContextProgress {
                generation,
                percent,
                label,
            } => {
                if generation == self.generation && self.context_pending {
                    self.context_progress = percent.min(100);
                    self.context_progress_label = label;
                    self.dirty = true;
                }
                return;
            }
            Reply::Profile { epoch, data } => {
                self.profile_pending = false;
                self.record_riot_sync("perfil", data.is_ok());
                if epoch != self.epoch {
                    self.dirty = true;
                    return;
                }
                self.profile_failed = data.is_err();
                if let Ok((profile, competitive, updates)) = data {
                    self.own_profile = Some(profile);
                    self.competitive = competitive;
                    self.competitive_updates = updates;
                    self.push_log(LogLevel::Success, "Perfil y rango actualizados");
                } else {
                    self.push_log(LogLevel::Warning, "No se pudo actualizar el perfil");
                }
            }
            Reply::History { epoch, data } => {
                self.history_pending = false;
                self.record_riot_sync("historial", data.is_ok());
                if epoch != self.epoch {
                    self.dirty = true;
                    return;
                }
                let in_postmatch = self
                    .state
                    .as_ref()
                    .is_some_and(|state| state.phase == GamePhase::PostMatch);
                let fresh_postmatch = in_postmatch && self.postmatch_history_refresh_pending;
                self.postmatch_history_refresh_pending = false;
                self.history_failed = data.is_err();
                if let Ok(entries) = data {
                    let entry_count = entries.len();
                    // La posición cambia al llegar nuevas partidas. Conservar la
                    // selección por los metadatos seguros, sin retener MatchID.
                    let selected = self
                        .history
                        .as_ref()
                        .and_then(|old| old.get(self.history_index));
                    let preserved =
                        selected.and_then(|old| entries.iter().position(|entry| entry == old));
                    self.history_index = if fresh_postmatch {
                        0
                    } else {
                        preserved.unwrap_or_else(|| {
                            self.history_index.min(entries.len().saturating_sub(1))
                        })
                    };
                    if preserved.is_none() && self.selected_tab == 3 {
                        self.detail = false;
                    }
                    self.follow_selection = true;
                    if fresh_postmatch {
                        self.player_index = entries
                            .first()
                            .and_then(|item| item.details.as_ref())
                            .and_then(|details| {
                                details.roster.iter().position(|player| player.is_self)
                            })
                            .unwrap_or(0);
                    }
                    let has_postmatch_detail = entries
                        .first()
                        .and_then(|item| item.details.as_ref())
                        .is_some();
                    self.history = Some(entries);
                    self.history_cached_at_ms = Some(now_ms());
                    self.postmatch_history_ready = fresh_postmatch && has_postmatch_detail;
                    if in_postmatch && !fresh_postmatch {
                        // Esta respuesta se pidió antes de terminar la partida.
                        // Programar otra consulta para incluir el resultado nuevo.
                        self.history_requested = false;
                    }
                    self.push_log(
                        LogLevel::Success,
                        format!("Historial actualizado: {entry_count} partidas"),
                    );
                } else {
                    self.postmatch_history_ready = false;
                    if in_postmatch && !fresh_postmatch {
                        self.history_requested = false;
                        self.history_failed = false;
                    }
                    self.push_log(LogLevel::Warning, "No se pudo actualizar el historial");
                }
            }
            Reply::Saved(result) => {
                let succeeded = result.is_ok();
                self.settings.saved(result);
                self.push_log(
                    if succeeded {
                        LogLevel::Success
                    } else {
                        LogLevel::Warning
                    },
                    if succeeded {
                        "Configuración guardada"
                    } else {
                        "No se pudo guardar la configuración"
                    },
                );
                if self.restart_requested {
                    if succeeded {
                        self.should_quit = true;
                    } else {
                        // No relanzar con un tema distinto al que el usuario
                        // acaba de elegir si el guardado falló.
                        self.restart_requested = false;
                    }
                }
                if self.quit_after_save {
                    if succeeded {
                        self.should_quit = true;
                    } else {
                        self.quit_after_save = false;
                    }
                }
            }
        }
        self.dirty = true;
    }

    fn schedule_data(&mut self, worker: &Worker) {
        if self.demo.is_some() {
            return;
        }
        let connected = self.state.as_ref().is_some_and(|state| state.client_found);
        // El perfil es barato y alimenta Resumen/Mi perfil en cualquier fase; no
        // debe quedar esperando detrás del enriquecimiento completo del roster.
        if connected
            && !self.profile_pending
            && !self.profile_requested
            && worker.submit(Request::Profile { epoch: self.epoch })
        {
            self.profile_pending = true;
            self.profile_requested = true;
            self.profile_failed = false;
            self.dirty = true;
        }
        if !self.context_pending
            && !self.context_requested
            && let Some(
                phase @ (GamePhase::PreGame
                | GamePhase::AgentSelect
                | GamePhase::InMatch
                | GamePhase::PostMatch),
            ) = self.state.as_ref().map(|info| info.phase)
            && worker.submit(Request::Context {
                phase,
                generation: self.generation,
            })
        {
            self.context_pending = true;
            self.context_requested = true;
            self.context_progress = 5;
            self.context_progress_label = "Preparando la consulta";
            self.dirty = true;
        }
        if !self.party_pending
            && self.party_attempts < MAX_PARTY_REFRESH_ATTEMPTS
            && self
                .party_retry_at
                .is_some_and(|retry_at| Instant::now() >= retry_at)
            && let Some(phase @ (GamePhase::PreGame | GamePhase::AgentSelect | GamePhase::InMatch)) =
                self.state.as_ref().map(|info| info.phase)
            && worker.submit(Request::Parties {
                phase,
                generation: self.generation,
            })
        {
            self.party_pending = true;
            self.party_attempts = self.party_attempts.saturating_add(1);
            self.party_retry_at = None;
            self.dirty = true;
        }
        if connected && !self.history_requested && !self.history_failed {
            self.request_history(worker);
        }
    }

    fn request_history(&mut self, worker: &Worker) {
        if self.demo.is_some() {
            return;
        }
        if !self.history_pending && worker.submit(Request::History { epoch: self.epoch }) {
            self.history_pending = true;
            self.history_requested = true;
            self.history_failed = false;
            self.postmatch_history_refresh_pending = self
                .state
                .as_ref()
                .is_some_and(|state| state.phase == GamePhase::PostMatch);
            self.dirty = true;
        }
    }

    fn key(&mut self, key: KeyEvent, worker: &Worker) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.request_quit(worker);
            return;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }
        match key.code {
            KeyCode::F(5) => {
                if self.settings.saving {
                    // El Reply::Saved completará el relanzamiento sin perder
                    // el tema que ya está en proceso de guardado.
                    self.restart_requested = true;
                    return;
                }
                if let Some(config) = self.settings.to_save() {
                    let result = crate::config::save(&config).map(|_| config).map_err(|_| ());
                    let succeeded = result.is_ok();
                    self.settings.saved(result);
                    self.push_log(
                        if succeeded {
                            LogLevel::Success
                        } else {
                            LogLevel::Warning
                        },
                        if succeeded {
                            "Configuración guardada antes de reiniciar"
                        } else {
                            "No se pudo guardar; reinicio cancelado"
                        },
                    );
                    if !succeeded {
                        return;
                    }
                }
                self.restart_requested = true;
                self.should_quit = true;
            }
            KeyCode::Char('q') => self.request_quit(worker),
            KeyCode::Esc => {
                if self.detail {
                    self.detail = false;
                    self.tracker_notice = false;
                    self.tracker_open_failed = false;
                    self.scroll = 0;
                } else {
                    self.select_tab(1);
                    if let Some(demo) = &mut self.demo {
                        demo.post = false;
                    }
                }
                self.focus = Focus::Content;
                self.follow_selection = true;
            }
            KeyCode::Char(c @ '1'..='5') => {
                self.select_tab(BASE_TABS[(c as u8 - b'1') as usize]);
                self.focus = Focus::Content;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Content {
                    Focus::Tabs
                } else {
                    Focus::Content
                };
                self.follow_selection = true;
            }
            KeyCode::Char('t') => {
                self.settings.cycle_theme();
                self.request_settings_save(worker);
            }
            KeyCode::Right | KeyCode::Char('l') => self.select_next(),
            KeyCode::Left | KeyCode::Char('h') => self.select_previous(),
            KeyCode::Enter if self.focus == Focus::Tabs => self.focus = Focus::Content,
            _ if self.focus == Focus::Tabs => return,
            KeyCode::Up | KeyCode::Char('k') if self.selected_tab == 4 => {
                self.settings.previous();
                self.follow_selection = true;
            }
            KeyCode::Down | KeyCode::Char('j') if self.selected_tab == 4 => {
                self.settings.select();
                self.follow_selection = true;
            }
            KeyCode::Char('+') | KeyCode::Char('=') if self.selected_tab == 4 => {
                let persist_theme = self.settings.selected == 0;
                self.settings.adjust(true);
                if persist_theme {
                    self.request_settings_save(worker);
                }
            }
            KeyCode::Char('-') if self.selected_tab == 4 => {
                let persist_theme = self.settings.selected == 0;
                self.settings.adjust(false);
                if persist_theme {
                    self.request_settings_save(worker);
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter
                if self.selected_tab == 4 && self.settings.selected == 3 =>
            {
                self.open_palette_editor()
            }
            KeyCode::Char(' ') | KeyCode::Enter if self.selected_tab == 4 => {
                let persist_theme = self.settings.selected == 0;
                self.settings.toggle();
                if persist_theme {
                    self.request_settings_save(worker);
                }
            }
            KeyCode::Char('s') if self.selected_tab == 4 => {
                if let Some(config) = self.settings.to_save()
                    && worker.submit(Request::Save(config))
                {
                    self.settings.saving = true;
                }
            }
            KeyCode::Char('r') if self.selected_tab == 4 => self.settings.discard(),
            KeyCode::Char('c') if self.selected_tab == 5 => self.logs.clear(),
            KeyCode::Char('r') if self.selected_tab == 3 => self.request_history(worker),
            KeyCode::Char('r')
                if self.selected_tab == 1
                    && self
                        .state
                        .as_ref()
                        .is_some_and(|state| state.phase == GamePhase::PostMatch) =>
            {
                self.postmatch_history_ready = false;
                self.history_requested = false;
                self.request_history(worker);
            }
            KeyCode::Char('r') => {
                if !self.context_pending {
                    self.context_requested = false;
                }
                if !self.profile_pending {
                    self.profile_requested = false;
                }
                if !self.history_pending && matches!(self.selected_tab, 0 | 2) {
                    self.request_history(worker);
                }
            }
            KeyCode::Char('p') if self.selected_tab == 1 && self.demo.is_some() => {
                let demo = self.demo.as_mut().unwrap();
                demo.post = !demo.post;
                self.detail = false;
                self.scroll = 0;
            }
            KeyCode::Char('[') if self.selected_tab == 1 && self.demo.is_some() => {
                self.round_page = self.round_page.saturating_sub(1)
            }
            KeyCode::Char(']') if self.selected_tab == 1 && self.demo.is_some() => {
                self.round_page = self.round_page.saturating_add(1)
            }
            KeyCode::Char('g')
                if self.selected_tab == 1 && self.demo.as_ref().is_some_and(|d| !d.post) =>
            {
                self.detail = true;
                self.tracker_notice = true;
                self.tracker_open_failed = false;
                self.follow_selection = true;
            }
            KeyCode::Char('g') if self.selected_tab == 1 && self.demo.is_none() => {
                self.detail = true;
                self.tracker_notice = true;
                self.tracker_open_failed = self
                    .selected_live_player()
                    .and_then(tracker_url)
                    .or_else(|| {
                        self.selected_postmatch_player()
                            .and_then(history_tracker_url)
                    })
                    .is_some_and(|url| open_tracker(&url).is_err());
                self.follow_selection = true;
            }
            KeyCode::Char('g') if self.selected_tab == 3 && self.detail => {
                self.tracker_notice = true;
                self.tracker_open_failed = self
                    .selected_history_player()
                    .and_then(history_tracker_url)
                    .is_some_and(|url| open_tracker(&url).is_err());
                self.follow_selection = true;
            }
            KeyCode::Enter if self.selected_tab == 0 && self.has_match_context() => {
                self.select_tab(1)
            }
            KeyCode::Enter if self.selected_tab == 3 => {
                if let Some(demo) = &mut self.demo {
                    demo.post = true;
                    self.select_tab(1);
                    self.player_index = 3;
                } else {
                    let opening = !self.detail;
                    self.detail = !self.detail;
                    if opening {
                        self.player_index = self
                            .history
                            .as_ref()
                            .and_then(|items| items.get(self.history_index))
                            .and_then(|item| item.details.as_ref())
                            .and_then(|details| {
                                details.roster.iter().position(|player| player.is_self)
                            })
                            .unwrap_or(0);
                    }
                }
                self.follow_selection = true;
            }
            KeyCode::Enter if self.selected_tab == 1 && self.has_selectable_roster() => {
                self.detail = !self.detail;
                self.tracker_notice = false;
                self.tracker_open_failed = false;
                self.follow_selection = true;
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
                if self.selected_tab == 3 && self.detail =>
            {
                let down = matches!(key.code, KeyCode::Down | KeyCode::Char('j'));
                let order = self
                    .history
                    .as_ref()
                    .and_then(|items| items.get(self.history_index))
                    .and_then(|item| item.details.as_ref())
                    .map_or_else(Vec::new, |details| {
                        completed_roster_display_order(&details.roster)
                    });
                self.player_index = adjacent_display_index(&order, self.player_index, down);
                self.follow_selection = true;
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
                if self.selected_tab == 3
                    || (self.selected_tab == 1 && self.has_selectable_roster()) =>
            {
                let down = matches!(key.code, KeyCode::Down | KeyCode::Char('j'));
                let (index, count) = if self.selected_tab == 3 {
                    (
                        &mut self.history_index,
                        self.demo.as_ref().map_or_else(
                            || self.history.as_ref().map_or(0, Vec::len),
                            |d| d.matches.len(),
                        ),
                    )
                } else {
                    let order = if let Some(demo) = &self.demo {
                        (0..demo.players.len()).collect()
                    } else if self.postmatch_history_ready {
                        self.history
                            .as_ref()
                            .and_then(|items| items.first())
                            .and_then(|item| item.details.as_ref())
                            .map_or_else(Vec::new, |details| {
                                completed_roster_display_order(&details.roster)
                            })
                    } else {
                        self.live_match
                            .as_ref()
                            .and_then(|context| context.roster.as_ref())
                            .map_or_else(Vec::new, |roster| {
                                live_roster_display_order(&roster.players)
                            })
                    };
                    self.player_index = adjacent_display_index(&order, self.player_index, down);
                    self.tracker_notice = false;
                    self.tracker_open_failed = false;
                    self.follow_selection = true;
                    self.dirty = true;
                    return;
                };
                if count > 0 {
                    *index = if down {
                        (*index + 1) % count
                    } else {
                        (*index + count - 1) % count
                    };
                }
                self.tracker_notice = false;
                self.tracker_open_failed = false;
                self.follow_selection = true;
            }
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
            KeyCode::Home => self.scroll = 0,
            _ => return,
        }
        self.dirty = true;
    }

    fn request_quit(&mut self, worker: &Worker) {
        if self.settings.saving {
            self.quit_after_save = true;
            return;
        }
        if let Some(config) = self.settings.to_save() {
            if worker.submit(Request::Save(config)) {
                self.settings.saving = true;
                self.quit_after_save = true;
                self.push_log(LogLevel::Info, "Guardando ajustes antes de salir");
                return;
            }
            self.push_log(LogLevel::Warning, "No se pudieron guardar los ajustes");
            return;
        }
        self.should_quit = true;
    }

    fn request_settings_save(&mut self, worker: &Worker) {
        if let Some(config) = self.settings.to_save()
            && worker.submit(Request::Save(config))
        {
            self.settings.saving = true;
        }
    }

    fn mouse(&mut self, mouse: MouseEvent, width: u16, height: u16) {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(3);
                self.follow_selection = false;
                self.dirty = true;
                return;
            }
            MouseEventKind::ScrollDown => {
                self.scroll = self.scroll.saturating_add(3);
                self.follow_selection = false;
                self.dirty = true;
                return;
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return,
        }

        let match_visible = self.match_tab_visible();
        let tab_rows = tab_rows(width);
        if mouse.row >= 1 && mouse.row <= tab_rows {
            if match_visible && mouse.row == 1 && mouse.column >= match_tab_x(width, width < 90) {
                self.select_tab(1);
                self.focus = Focus::Content;
                return;
            }
            let row = usize::from(mouse.row.saturating_sub(1));
            let indices = tab_indices(width, row, match_visible);
            let mut x = 1_u16;
            for &index in indices {
                let label_width = u16::try_from(tab_text(index, width < 90).chars().count() + 1)
                    .unwrap_or(u16::MAX);
                if mouse.column >= x && mouse.column < x.saturating_add(label_width) {
                    self.select_tab(index);
                    self.focus = Focus::Content;
                    return;
                }
                x = x.saturating_add(label_width);
            }
        }

        let chart_rows = if self.selected_tab == 3
            && !self.detail
            && width >= 70
            && height >= 22
            && self.history.as_ref().is_some_and(|items| {
                items.iter().filter(|item| item.rr_change.is_some()).count() >= 2
            }) {
            9
        } else {
            0
        };
        let body_y = 2 + tab_rows + chart_rows;
        if mouse.row < body_y {
            return;
        }
        let line = usize::from(mouse.row - body_y + self.scroll);
        if self.selected_tab == 3 {
            let count = self.history.as_ref().map_or(0, Vec::len);
            if (2..2 + count).contains(&line) {
                self.history_index = line - 2;
                self.detail = false;
                self.follow_selection = true;
                self.dirty = true;
            }
        } else if self.selected_tab == 4 {
            let screen = view::content(self, width.saturating_sub(2));
            let selected = screen
                .setting_rows
                .iter()
                .position(|row| *row == Some(line));
            if let Some(selected) = selected {
                self.settings.selected = selected;
                if selected == 3 {
                    self.open_palette_editor();
                }
                self.follow_selection = true;
                self.dirty = true;
            }
        }
    }
}

fn tracker_url(player: &RosterPlayer) -> Option<String> {
    let DataAvailability::Available(identity) = &player.identity else {
        return None;
    };
    let (name, tag) = identity.rsplit_once('#')?;
    if name.is_empty() || tag.is_empty() || identity == "Tú" {
        return None;
    }
    tracker_url_for_riot_id(identity)
}

fn adjacent_display_index(order: &[usize], current: usize, down: bool) -> usize {
    let Some(position) = order.iter().position(|index| *index == current) else {
        return order.first().copied().unwrap_or(0);
    };
    let next = if down {
        (position + 1) % order.len()
    } else {
        (position + order.len() - 1) % order.len()
    };
    order[next]
}

fn completed_roster_display_order(roster: &[CompletedRosterPlayer]) -> Vec<usize> {
    [
        CompletedPlayerSide::Ally,
        CompletedPlayerSide::Enemy,
        CompletedPlayerSide::Participant,
    ]
    .into_iter()
    .flat_map(|side| {
        roster
            .iter()
            .enumerate()
            .filter_map(move |(index, player)| (player.side == side).then_some(index))
    })
    .collect()
}

fn live_roster_display_order(roster: &[RosterPlayer]) -> Vec<usize> {
    [RosterSide::Ally, RosterSide::Enemy, RosterSide::Participant]
        .into_iter()
        .flat_map(|side| {
            roster
                .iter()
                .enumerate()
                .filter_map(move |(index, player)| (player.side == side).then_some(index))
        })
        .collect()
}

fn history_tracker_url(player: &CompletedRosterPlayer) -> Option<String> {
    tracker_url_for_riot_id(player.riot_id.as_deref()?)
}

fn tracker_url_for_riot_id(identity: &str) -> Option<String> {
    let (name, tag) = identity.rsplit_once('#')?;
    if name.is_empty() || tag.is_empty() {
        return None;
    }
    Some(format!(
        "https://tracker.gg/valorant/profile/riot/{}/overview",
        encode_path_segment(identity)
    ))
}

fn encode_path_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    encoded
}

fn open_palette_folder() -> io::Result<()> {
    let palette = theme::palette_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA no está disponible"))?;
    let directory = palette
        .parent()
        .ok_or_else(|| io::Error::other("ruta de paleta inválida"))?;
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

        let operation = "open\0".encode_utf16().collect::<Vec<_>>();
        let target = directory
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        use std::os::windows::ffi::OsStrExt;
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize > 32 {
            Ok(())
        } else {
            Err(io::Error::other("Windows no pudo abrir la carpeta"))
        }
    }
    #[cfg(target_os = "macos")]
    {
        ProcessCommand::new("open").arg(directory).spawn()?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        ProcessCommand::new("xdg-open").arg(directory).spawn()?;
        Ok(())
    }
}

fn open_tracker(url: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

        let operation = "open\0".encode_utf16().collect::<Vec<_>>();
        let target = url
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // ShellExecuteW usa directamente la asociación HTTPS de Windows y no
        // interpreta los `%` de la URL como variables de CMD.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize > 32 {
            Ok(())
        } else {
            Err(io::Error::other("Windows no pudo abrir el navegador"))
        }
    }
    #[cfg(target_os = "macos")]
    {
        ProcessCommand::new("open").arg(url).spawn()?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        ProcessCommand::new("xdg-open").arg(url).spawn()?;
        Ok(())
    }
}

/// Restaura la terminal incluso si falla la inicialización o hay un panic.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}

#[cfg(target_os = "windows")]
struct ConsoleBufferGuard {
    handle: windows_sys::Win32::Foundation::HANDLE,
    original_size: windows_sys::Win32::System::Console::COORD,
}

#[cfg(target_os = "windows")]
impl ConsoleBufferGuard {
    fn new() -> Option<Self> {
        use windows_sys::Win32::System::Console::{
            CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo, GetStdHandle, STD_OUTPUT_HANDLE,
        };

        // SAFETY: las funciones reciben un handle de stdout y un puntero válido
        // durante toda la llamada; si el host no expone una consola Win32 se omite.
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle.is_null() || handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return None;
            }
            let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
            if GetConsoleScreenBufferInfo(handle, &mut info) == 0 {
                return None;
            }
            let guard = Self {
                handle,
                original_size: info.dwSize,
            };
            guard.fit_visible();
            Some(guard)
        }
    }

    fn fit_visible(&self) {
        use windows_sys::Win32::System::Console::{
            CONSOLE_SCREEN_BUFFER_INFO, COORD, GetConsoleScreenBufferInfo,
            SetConsoleScreenBufferSize,
        };

        // SAFETY: self.handle permanece válido mientras stdout siga asociado a
        // esta consola y `info` vive hasta terminar la llamada.
        unsafe {
            let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
            if GetConsoleScreenBufferInfo(self.handle, &mut info) == 0 {
                return;
            }
            let visible = COORD {
                X: info.srWindow.Right - info.srWindow.Left + 1,
                Y: info.srWindow.Bottom - info.srWindow.Top + 1,
            };
            let _ = SetConsoleScreenBufferSize(self.handle, visible);
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for ConsoleBufferGuard {
    fn drop(&mut self) {
        // SAFETY: se restaura el tamaño capturado del mismo handle. Un fallo no
        // impide que TerminalGuard restaure el modo normal de la terminal.
        unsafe {
            let _ = windows_sys::Win32::System::Console::SetConsoleScreenBufferSize(
                self.handle,
                self.original_size,
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn fit_console_buffer_to_window() {
    use windows_sys::Win32::System::Console::{
        CONSOLE_SCREEN_BUFFER_INFO, COORD, GetConsoleScreenBufferInfo, GetStdHandle,
        STD_OUTPUT_HANDLE, SetConsoleScreenBufferSize,
    };

    // SAFETY: solo consulta y ajusta el búfer correspondiente a stdout.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() || handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return;
        }
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        if GetConsoleScreenBufferInfo(handle, &mut info) == 0 {
            return;
        }
        let visible = COORD {
            X: info.srWindow.Right - info.srWindow.Left + 1,
            Y: info.srWindow.Bottom - info.srWindow.Top + 1,
        };
        let _ = SetConsoleScreenBufferSize(handle, visible);
    }
}

pub(crate) fn run(config: Config, demo: bool) -> io::Result<()> {
    use io::IsTerminal;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "el dashboard requiere una terminal interactiva; usa watch --once para salida de texto",
        ));
    }
    enable_raw_mode()?;
    let terminal_guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        crossterm::terminal::SetTitle("SPIKE"),
        EnterAlternateScreen,
        EnableMouseCapture,
        Clear(ClearType::Purge),
        MoveTo(0, 0)
    )?;
    #[cfg(target_os = "windows")]
    let buffer_guard = ConsoleBufferGuard::new();
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let restart = run_loop(&mut terminal, config, demo)?;
    drop(terminal);
    #[cfg(target_os = "windows")]
    drop(buffer_guard);
    drop(terminal_guard);
    if restart {
        relaunch_current()?;
    }
    Ok(())
}

fn relaunch_current() -> io::Result<()> {
    ProcessCommand::new(env::current_exe()?)
        .args(env::args_os().skip(1))
        .spawn()?;
    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
    demo: bool,
) -> io::Result<bool> {
    let worker = if demo {
        Worker::demo()?
    } else {
        Worker::start()?
    };
    let mut app = App::new(&config);
    app.reload_palette();
    if !demo && let Some(cached) = worker::load_cached_history() {
        let count = cached.items.len();
        app.history = Some(cached.items);
        app.history_cached_at_ms = Some(cached.saved_at_ms);
        app.push_log(
            LogLevel::Info,
            format!("Historial recuperado de caché: {count} partidas"),
        );
    }
    if demo {
        app.demo = Some(demo::Demo::default());
        app.selected_tab = 1;
        // Las capturas de la demo deben ser reproducibles, sin depender del
        // tema que cada persona tenga guardado localmente.
        app.settings.draft.theme = crate::config::Theme::Dark;
        app.metrics = metrics::ProcessMetrics::demo();
        app.push_log(LogLevel::Info, "Demo: datos ficticios cargados");
        app.push_log(LogLevel::Success, "Demo: historial Ranked preparado");
        app.push_log(LogLevel::Success, "Demo: roster de partida preparado");
        app.push_log(LogLevel::Info, "Demo: métricas locales simuladas");
    }
    let mut last_refresh: Option<Instant> = None;

    while !app.should_quit {
        app.tick();
        loop {
            match worker.receive() {
                Ok(reply) => app.apply(reply),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err(io::Error::other(
                        "el trabajador de datos se detuvo; vuelve a abrir el dashboard",
                    ));
                }
            }
        }
        if app.demo.is_none()
            && !app.observation_pending
            && last_refresh.is_none_or(|at| at.elapsed() >= app.settings.active.interval)
            && worker.submit(Request::Observe {
                log: app.settings.active.log_transitions,
            })
        {
            app.observation_pending = true;
            last_refresh = Some(Instant::now());
        }
        app.schedule_data(&worker);
        if app.dirty {
            terminal.draw(|frame| render(frame.area(), frame, &mut app))?;
            app.dirty = false;
        }
        if event::poll(INPUT_TIMEOUT)? {
            match event::read()? {
                Event::Key(key) => app.key(key, &worker),
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    app.mouse(mouse, size.width, size.height)
                }
                Event::Resize(_, _) => {
                    #[cfg(target_os = "windows")]
                    fit_console_buffer_to_window();
                    app.dirty = true;
                    app.follow_selection = true;
                }
                _ => {}
            }
        }
    }
    Ok(app.restart_requested)
}

#[cfg(test)]
fn content_for(app: &App) -> String {
    view::content(app, 78)
        .lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
fn history_content(app: &App) -> String {
    let mut history = App::new(&Config::default());
    history.selected_tab = 3;
    history.history = app.history.clone();
    history.history_failed = app.history_failed;
    content_for(&history)
}

fn relative_time(started_at_ms: u64) -> String {
    let now = now_ms();
    match now.saturating_sub(started_at_ms) / 1_000 {
        0..=59 => "ahora".into(),
        60..=3_599 => format!("hace {} min", now.saturating_sub(started_at_ms) / 60_000),
        3_600..=86_399 => format!("hace {} h", now.saturating_sub(started_at_ms) / 3_600_000),
        _ => format!("hace {} d", now.saturating_sub(started_at_ms) / 86_400_000),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::GameState,
        providers::{
            capabilities::{Confidence, GamePhase},
            live_match::{LivePartyUpdate, parse_live_match},
        },
    };

    #[test]
    fn match_tab_is_contextual_and_navigation_keeps_five_base_views() {
        let mut app = App::new(&Config::default());
        assert!(!app.match_tab_visible());
        assert_eq!(app.navigation_tabs(), BASE_TABS);
        app.select_tab(1);
        assert_eq!(app.selected_tab, 0);
        app.select_previous();
        assert_eq!(app.selected_tab, TABS.len() - 1);
        app.select_next();
        assert_eq!(app.selected_tab, 0);

        app.update_state(StateInfo::new(
            GamePhase::InMatch,
            GameState::GameOpen,
            Confidence::High,
            "local-client",
            true,
            true,
        ));
        assert!(app.match_tab_visible());
        assert_eq!(app.navigation_tabs(), [0, 2, 3, 4, 5, 1]);
        app.select_tab(1);
        app.update_state(StateInfo::new(
            GamePhase::Idle,
            GameState::Idle,
            Confidence::High,
            "local-client",
            true,
            false,
        ));
        assert!(!app.match_tab_visible());
        assert_eq!(app.selected_tab, 0);
    }

    #[test]
    fn splash_finishes_only_after_three_seconds() {
        let mut app = App::new(&Config::default());
        app.tick();
        assert!(!app.splash_complete);

        app.splash_started = Instant::now() - SPLASH_DURATION;
        app.tick();
        assert!(app.splash_complete);
    }

    #[test]
    fn f5_requests_a_clean_restart() {
        let worker = Worker::demo().unwrap();
        let mut app = App::new(&Config::default());
        app.key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE), &worker);
        assert!(app.should_quit);
        assert!(app.restart_requested);
    }

    #[test]
    fn f5_waits_for_an_in_flight_theme_save_before_restarting() {
        let worker = Worker::demo().unwrap();
        let mut app = App::new(&Config::default());
        app.settings.draft.theme = crate::config::Theme::Light;
        app.settings.saving = true;

        app.key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE), &worker);
        assert!(app.restart_requested);
        assert!(!app.should_quit);

        let saved = app.settings.draft.clone();
        app.apply(Reply::Saved(Ok(saved)));
        assert!(app.should_quit);
        assert_eq!(app.settings.active.theme, crate::config::Theme::Light);
    }

    #[test]
    fn theme_changes_are_saved_and_pending_settings_finish_before_quitting() {
        let worker = Worker::demo().unwrap();
        let mut app = App::new(&Config::default());

        app.key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &worker,
        );
        assert!(app.settings.saving);
        let selected = app.settings.draft.clone();
        app.apply(Reply::Saved(Ok(selected.clone())));
        assert_eq!(app.settings.active.theme, selected.theme);

        app.settings.draft.theme = crate::config::Theme::Mono;
        app.key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            &worker,
        );
        assert!(app.quit_after_save && !app.should_quit);
        let pending = app.settings.draft.clone();
        app.apply(Reply::Saved(Ok(pending)));
        assert!(app.should_quit);
    }

    #[test]
    fn context_progress_tracks_only_the_current_match_generation() {
        let mut app = App::new(&Config::default());
        app.context_pending = true;
        app.generation = 7;
        app.apply(Reply::ContextProgress {
            generation: 7,
            percent: 45,
            label: "Partida detectada",
        });
        assert_eq!(app.context_progress, 45);
        assert_eq!(app.context_progress_label, "Partida detectada");

        app.apply(Reply::ContextProgress {
            generation: 6,
            percent: 90,
            label: "Respuesta antigua",
        });
        assert_eq!(app.context_progress, 45);
    }

    #[test]
    fn progressive_party_reply_updates_the_existing_roster() {
        let mut app = App::new(&Config::default());
        app.generation = 4;
        app.live_match = Some(
            parse_live_match(
                &serde_json::json!({
                    "ModeID":"/Game/GameModes/Bomb/Bomb",
                    "MapID":"/Game/Maps/Ascent/Ascent",
                    "Players":[
                        {"Subject":"me", "TeamID":"Blue"},
                        {"Subject":"enemy", "TeamID":"Red"}
                    ]
                }),
                "me",
            )
            .unwrap(),
        );
        app.party_pending = true;

        app.apply(Reply::Parties {
            generation: 4,
            data: Ok(LivePartyUpdate {
                premades: vec![
                    DataAvailability::Available("Solo".into()),
                    DataAvailability::Available("Solo".into()),
                ],
                complete: true,
            }),
        });

        let roster = app.live_match.unwrap().roster.unwrap();
        assert!(
            roster
                .players
                .iter()
                .all(|player| { player.premade == DataAvailability::Available("Solo".to_owned()) })
        );
        assert!(!app.party_pending);
        assert!(app.party_retry_at.is_none());
    }

    #[test]
    fn tracker_links_use_only_public_riot_ids_and_encode_the_path() {
        let player = |identity| RosterPlayer {
            side: crate::models::roster::RosterSide::Ally,
            slot: 1,
            is_self: true,
            identity,
            agent: DataAvailability::NotAvailable,
            rank: DataAvailability::NotAvailable,
            level: DataAvailability::NotAvailable,
            premade: DataAvailability::NotAvailable,
            stats: DataAvailability::NotAvailable,
        };

        let visible = player(DataAvailability::Available("Ñorte uno#LAS".into()));
        assert_eq!(
            tracker_url(&visible).as_deref(),
            Some("https://tracker.gg/valorant/profile/riot/%C3%91orte%20uno%23LAS/overview")
        );
        assert!(tracker_url(&player(DataAvailability::Hidden)).is_none());
        assert!(tracker_url(&player(DataAvailability::Available("Tú".into()))).is_none());

        let historical = CompletedRosterPlayer {
            side: crate::providers::match_detail::CompletedPlayerSide::Enemy,
            slot: 1,
            is_self: false,
            name: "Ñorte uno#LAS".into(),
            riot_id: Some("Ñorte uno#LAS".into()),
            agent: "Omen".into(),
            rank: None,
            stats: Default::default(),
            rounds_played: 20,
            premade: None,
        };
        assert_eq!(
            history_tracker_url(&historical).as_deref(),
            Some("https://tracker.gg/valorant/profile/riot/%C3%91orte%20uno%23LAS/overview")
        );
    }

    #[test]
    fn historical_player_navigation_follows_the_visible_team_order() {
        let player = |side, slot| CompletedRosterPlayer {
            side,
            slot,
            is_self: false,
            name: format!("Jugador {slot}"),
            riot_id: None,
            agent: "Sova".into(),
            rank: None,
            stats: Default::default(),
            rounds_played: 20,
            premade: None,
        };
        let roster = vec![
            player(CompletedPlayerSide::Ally, 1),
            player(CompletedPlayerSide::Enemy, 1),
            player(CompletedPlayerSide::Enemy, 2),
            player(CompletedPlayerSide::Ally, 2),
            player(CompletedPlayerSide::Ally, 3),
        ];

        let order = completed_roster_display_order(&roster);
        assert_eq!(order, vec![0, 3, 4, 1, 2]);
        assert_eq!(adjacent_display_index(&order, 0, true), 3);
        assert_eq!(adjacent_display_index(&order, 3, true), 4);
        assert_eq!(adjacent_display_index(&order, 0, false), 2);
    }

    #[test]
    fn mouse_selects_tabs_history_rows_settings_and_scrolls() {
        let mut app = App::new(&Config::default());
        let click = |column, row| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };

        app.mouse(click(17, 1), 80, 24);
        assert_eq!(app.selected_tab, 2);

        app.mouse(click(5, 2), 60, 24);
        assert_eq!(app.selected_tab, 4);

        app.history = Some(vec![history_item(200), history_item(100)]);
        app.select_tab(3);
        app.mouse(click(5, 7), 80, 24);
        assert_eq!(app.history_index, 1);

        app.select_tab(4);
        app.mouse(click(5, 12), 80, 24);
        assert_eq!(app.settings.selected, 2);

        app.mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 5,
                row: 10,
                modifiers: KeyModifiers::NONE,
            },
            80,
            24,
        );
        assert_eq!(app.scroll, 3);
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
        app.history = Some(vec![history_item(0)]);

        let text = history_content(&app);
        assert!(text.contains("HISTORIAL RANKED"));
        assert!(!text.contains("match"));
    }

    fn phase_info(phase: GamePhase) -> StateInfo {
        StateInfo::new(
            phase,
            GameState::Idle,
            Confidence::High,
            "test",
            true,
            false,
        )
    }

    fn history_item(started_at_ms: u64) -> HistoryItem {
        HistoryItem {
            entry: HistoryEntry {
                queue: "competitivo".into(),
                started_at_ms,
            },
            details: None,
            rr_change: None,
            rr_after: None,
        }
    }

    fn post_match(round_count: u32) -> PostMatch {
        PostMatch {
            mode: GameMode::Competitive,
            map: "Ascent".into(),
            agent: "Sova".into(),
            outcome: MatchOutcome::Win,
            stats: PlayerMatchStats {
                kills: 15,
                deaths: 10,
                assists: 4,
                combat_score: Some(3_988),
                ..Default::default()
            },
            own_score: Some(13),
            opponent_score: Some(9),
            rounds: (1..=round_count)
                .map(|number| PostRound {
                    number,
                    result: "eliminación".into(),
                    kills: 1,
                    deaths: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn ignores_late_context_and_history_from_a_closed_session() {
        let mut app = App::new(&Config::default());
        app.update_state(phase_info(GamePhase::Idle));
        let epoch = app.epoch;
        app.update_state(phase_info(GamePhase::ClientClosed));
        app.apply(Reply::Profile {
            epoch,
            data: Ok((OwnProfile { level: 50, xp: 10 }, None, vec![])),
        });
        app.apply(Reply::History {
            epoch,
            data: Ok(vec![history_item(0)]),
        });
        assert!(app.own_profile.is_none());
        assert!(app.history.is_none());
        assert!(!app.context_pending && !app.history_pending);
    }

    #[test]
    fn preserves_last_history_and_postmatch_when_refresh_fails_or_phase_expires() {
        let mut app = App::new(&Config::default());
        app.update_state(phase_info(GamePhase::PostMatch));
        app.apply(Reply::Context {
            generation: app.generation,
            data: Ok(Context::Completed(post_match(2))),
        });
        app.update_state(phase_info(GamePhase::GameOpen));
        assert_eq!(
            app.completed_match.as_ref().map(|item| item.map.as_str()),
            Some("Ascent")
        );
        app.history = Some(vec![history_item(0)]);
        app.apply(Reply::History {
            epoch: app.epoch,
            data: Err(()),
        });
        assert!(history_content(&app).contains("HISTORIAL RANKED"));
        assert!(history_content(&app).contains("Última consulta"));
        app.update_state(phase_info(GamePhase::ClientClosed));
        assert!(app.completed_match.is_none());
    }

    #[test]
    fn unchanged_observations_do_not_redraw_but_recovery_does() {
        let mut app = App::new(&Config::default());
        app.update_state(phase_info(GamePhase::Idle));
        app.dirty = false;
        app.apply(Reply::Observed {
            state: Ok(phase_info(GamePhase::Idle)),
            log_failed: false,
        });
        assert!(!app.dirty);
        app.refresh_failed = true;
        app.apply(Reply::Observed {
            state: Ok(phase_info(GamePhase::Idle)),
            log_failed: false,
        });
        assert!(app.dirty && !app.refresh_failed);
    }

    #[test]
    fn history_refresh_preserves_selected_match_and_closes_removed_detail() {
        let mut app = App::new(&Config::default());
        app.selected_tab = 3;
        app.history = Some(vec![history_item(200), history_item(100)]);
        app.history_index = 1;
        app.detail = true;
        app.apply(Reply::History {
            epoch: app.epoch,
            data: Ok(vec![
                history_item(300),
                history_item(200),
                history_item(100),
            ]),
        });
        assert_eq!(app.history_index, 2);
        assert!(app.detail);
        app.apply(Reply::History {
            epoch: app.epoch,
            data: Err(()),
        });
        assert_eq!(app.history_index, 2);
        assert!(app.detail);
        app.apply(Reply::History {
            epoch: app.epoch,
            data: Ok(vec![history_item(400), history_item(300)]),
        });
        assert!(!app.detail);
        assert!(app.history_index < app.history.as_ref().unwrap().len());
        app.apply(Reply::History {
            epoch: app.epoch,
            data: Ok(vec![]),
        });
        assert_eq!(app.history_index, 0);
        assert!(!app.detail);
    }

    #[test]
    fn duplicate_refreshes_are_suppressed_and_navigation_stays_available() {
        use std::sync::mpsc;
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = Worker::spawn(move |request, _| match request {
            Request::History { epoch } => {
                started_tx.send("history").unwrap();
                release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                Reply::History {
                    epoch,
                    data: Ok(vec![]),
                }
            }
            Request::Save(config) => {
                started_tx.send("save").unwrap();
                Reply::Saved(Ok(config))
            }
            _ => panic!("unexpected test request"),
        })
        .unwrap();
        let mut app = App::new(&Config::default());
        app.selected_tab = 3;
        app.request_history(&worker);
        assert_eq!(
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            "history"
        );
        for _ in 0..20 {
            app.key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                &worker,
            );
        }
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &worker);
        assert_eq!(app.selected_tab, 3);
        assert!(app.focus == Focus::Tabs);
        app.key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &worker,
        );
        assert!(app.should_quit);
        // Barrera simulada, sin escribir configuración ni consultar proveedores.
        assert!(worker.submit(Request::Save(Config::default())));
        release_tx.send(()).unwrap();
        assert_eq!(
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            "save"
        );
    }

    #[test]
    fn renders_tabs_at_small_sizes_and_scrolls_long_postmatch_tables() {
        use ratatui::backend::TestBackend;
        let mut app = App::new(&Config::default());
        app.splash_complete = true;
        for (width, height) in [(1, 1), (32, 10), (80, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for tab in 0..TABS.len() {
                app.selected_tab = tab;
                terminal
                    .draw(|frame| render(frame.area(), frame, &mut app))
                    .unwrap();
            }
        }
        app.selected_tab = 1;
        app.completed_match = Some(post_match(30));
        app.scroll = 30;
        app.follow_selection = false;
        let mut terminal = Terminal::new(TestBackend::new(80, 15)).unwrap();
        terminal
            .draw(|frame| render(frame.area(), frame, &mut app))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains(" 30") && text.contains("eliminación"));
    }
}
