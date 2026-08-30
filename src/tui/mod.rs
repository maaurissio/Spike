//! Interfaz terminal interactiva, sin I/O remoto durante el renderizado.

mod demo;
mod settings;
mod theme;
mod view;
mod worker;

use std::{
    io,
    process::Command as ProcessCommand,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    config::Config,
    models::{
        GameMode, MatchOutcome, PlayerMatchStats,
        roster::{DataAvailability, RosterPlayer},
    },
    providers::{
        StateInfo,
        capabilities::GamePhase,
        history::HistoryEntry,
        live_match::LiveMatchContext,
        match_detail::CompletedMatch,
        profile::{CompetitiveProfile, CompetitiveUpdate, OwnProfile},
    },
};
use settings::Settings;
use view::render;
use worker::{Context, Reply, Request, Worker};

const TABS: [&str; 5] = ["Resumen", "Partida", "Mi perfil", "Historial", "Ajustes"];
const INPUT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryDetails {
    map: String,
    agent: String,
    outcome: MatchOutcome,
    rounds_played: u32,
    stats: PlayerMatchStats,
    own_score: Option<u32>,
    opponent_score: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryItem {
    entry: HistoryEntry,
    details: Option<HistoryDetails>,
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

struct App {
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
    history_failed: bool,
    profile_pending: bool,
    profile_requested: bool,
    profile_failed: bool,
    settings: Settings,
    observation_pending: bool,
    context_pending: bool,
    context_requested: bool,
    context_failed: bool,
    history_pending: bool,
    generation: u64,
    epoch: u64,
    log_failed: bool,
    scroll: u16,
    refresh_failed: bool,
    dirty: bool,
    should_quit: bool,
}

impl App {
    fn new(config: &Config) -> Self {
        Self {
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
            history_failed: false,
            profile_pending: false,
            profile_requested: false,
            profile_failed: false,
            settings: Settings::new(config),
            observation_pending: false,
            context_pending: false,
            context_requested: false,
            context_failed: false,
            history_pending: false,
            generation: 0,
            epoch: 0,
            log_failed: false,
            scroll: 0,
            refresh_failed: false,
            dirty: true,
            should_quit: false,
        }
    }

    fn select_next(&mut self) {
        self.select_tab((self.selected_tab + 1) % TABS.len());
    }

    fn select_previous(&mut self) {
        self.select_tab((self.selected_tab + TABS.len() - 1) % TABS.len());
    }

    fn select_tab(&mut self, tab: usize) {
        self.selected_tab = tab;
        self.scroll = 0;
        self.detail = false;
        self.tracker_notice = false;
        self.tracker_open_failed = false;
        self.follow_selection = true;
        self.dirty = true;
    }

    fn has_match_context(&self) -> bool {
        self.demo.is_some()
            || self.live_match.is_some()
            || self.completed_match.is_some()
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

    fn selected_live_player(&self) -> Option<&RosterPlayer> {
        self.live_match
            .as_ref()?
            .roster
            .as_ref()?
            .players
            .get(self.player_index)
    }

    fn has_selectable_roster(&self) -> bool {
        self.demo.as_ref().is_some_and(|demo| !demo.post) || self.selected_live_player().is_some()
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
        if phase_changed {
            self.scroll = 0;
            self.detail = false;
            self.follow_selection = true;
            self.generation = self.generation.wrapping_add(1);
            self.context_requested = false;
            self.context_failed = false;
            self.live_match = None;
            // El último resumen permanece disponible tras volver al menú.
            if matches!(state.phase, GamePhase::InMatch | GamePhase::ClientClosed) {
                self.completed_match = None;
            }
            if state.phase == GamePhase::ClientClosed {
                self.epoch = self.epoch.wrapping_add(1);
                self.own_profile = None;
                self.competitive = None;
                self.competitive_updates.clear();
                self.profile_pending = false;
                self.profile_requested = false;
                self.profile_failed = false;
                self.history = None;
                self.history_index = 0;
                self.history_failed = false;
            }
        }
        self.dirty |= changed || self.refresh_failed;
        self.state = Some(state);
        self.refresh_failed = false;
        phase_changed
    }

    fn mark_refresh_failed(&mut self) {
        if !self.refresh_failed {
            self.refresh_failed = true;
            self.dirty = true;
        }
    }

    fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::Observed { state, log_failed } => {
                self.observation_pending = false;
                self.dirty |= self.log_failed != log_failed;
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
                if generation != self.generation {
                    self.dirty = true;
                    return;
                }
                self.context_failed = data.is_err();
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
                    }
                    Ok(Context::Completed(summary)) => self.completed_match = Some(summary),
                    Err(()) => {} // Conservar el último dato de esta sesión al fallar un refresh.
                }
            }
            Reply::Profile { epoch, data } => {
                self.profile_pending = false;
                if epoch != self.epoch {
                    self.dirty = true;
                    return;
                }
                self.profile_failed = data.is_err();
                if let Ok((profile, competitive, updates)) = data {
                    self.own_profile = Some(profile);
                    self.competitive = competitive;
                    self.competitive_updates = updates;
                }
            }
            Reply::History { epoch, data } => {
                self.history_pending = false;
                if epoch != self.epoch {
                    self.dirty = true;
                    return;
                }
                self.history_failed = data.is_err();
                if let Ok(entries) = data {
                    // La posición cambia al llegar nuevas partidas. Conservar la
                    // selección por los metadatos seguros, sin retener MatchID.
                    let selected = self
                        .history
                        .as_ref()
                        .and_then(|old| old.get(self.history_index));
                    let preserved =
                        selected.and_then(|old| entries.iter().position(|entry| entry == old));
                    self.history_index = preserved
                        .unwrap_or_else(|| self.history_index.min(entries.len().saturating_sub(1)));
                    if preserved.is_none() && self.selected_tab == 3 {
                        self.detail = false;
                    }
                    self.follow_selection = true;
                    self.history = Some(entries);
                }
            }
            Reply::Saved(result) => self.settings.saved(result),
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
            self.dirty = true;
        }
        if connected && self.history.is_none() && !self.history_failed {
            self.request_history(worker);
        }
    }

    fn request_history(&mut self, worker: &Worker) {
        if self.demo.is_some() {
            return;
        }
        if !self.history_pending && worker.submit(Request::History { epoch: self.epoch }) {
            self.history_pending = true;
            self.history_failed = false;
            self.dirty = true;
        }
    }

    fn key(&mut self, key: KeyEvent, worker: &Worker) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
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
                self.select_tab((c as u8 - b'1') as usize);
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
            KeyCode::Char('t') => self.settings.cycle_theme(),
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
                self.settings.adjust(true)
            }
            KeyCode::Char('-') if self.selected_tab == 4 => self.settings.adjust(false),
            KeyCode::Char(' ') | KeyCode::Enter if self.selected_tab == 4 => self.settings.toggle(),
            KeyCode::Char('s') if self.selected_tab == 4 => {
                if let Some(config) = self.settings.to_save()
                    && worker.submit(Request::Save(config))
                {
                    self.settings.saving = true;
                }
            }
            KeyCode::Char('r') if self.selected_tab == 4 => self.settings.discard(),
            KeyCode::Char('r') if self.selected_tab == 3 => self.request_history(worker),
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
                } else {
                    self.detail = !self.detail;
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
                    let count = self.demo.as_ref().map_or_else(
                        || {
                            self.live_match
                                .as_ref()
                                .and_then(|context| context.roster.as_ref())
                                .map_or(0, |roster| roster.players.len())
                        },
                        |demo| demo.players.len(),
                    );
                    (&mut self.player_index, count)
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

    fn mouse(&mut self, mouse: MouseEvent, width: u16) {
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

        let tab_rows = if width < 72 { 2 } else { 1 };
        if mouse.row == 1 || (tab_rows == 2 && mouse.row == 2) {
            let row = usize::from(mouse.row.saturating_sub(1));
            let indices: Vec<usize> = if tab_rows == 1 {
                (0..TABS.len()).collect()
            } else if row == 0 {
                (0..3).collect()
            } else {
                (3..5).collect()
            };
            let mut x = 1_u16;
            for index in indices {
                let label_width = u16::try_from(format!(" {} {} ", index + 1, TABS[index]).len())
                    .unwrap_or(u16::MAX);
                if mouse.column >= x && mouse.column < x.saturating_add(label_width) {
                    self.select_tab(index);
                    self.focus = Focus::Content;
                    return;
                }
                x = x.saturating_add(label_width);
            }
        }

        let body_y = 2 + tab_rows;
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

fn open_tracker(url: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        ProcessCommand::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(url)
            .spawn()?;
        Ok(())
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

pub(crate) fn run(config: Config, demo: bool) -> io::Result<()> {
    use io::IsTerminal;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "el dashboard requiere una terminal interactiva; usa watch --once para salida de texto",
        ));
    }
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    run_loop(&mut terminal, config, demo)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
    demo: bool,
) -> io::Result<()> {
    let worker = if demo {
        Worker::demo()?
    } else {
        Worker::start()?
    };
    let mut app = App::new(&config);
    if demo {
        app.demo = Some(demo::Demo::default());
        app.selected_tab = 1;
    }
    let mut last_refresh: Option<Instant> = None;

    while !app.should_quit {
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
                Event::Mouse(mouse) => app.mouse(mouse, terminal.size()?.width),
                Event::Resize(_, _) => {
                    app.dirty = true;
                    app.follow_selection = true;
                }
                _ => {}
            }
        }
    }
    Ok(())
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

        app.mouse(click(12, 1), 80);
        assert_eq!(app.selected_tab, 1);

        app.mouse(click(5, 2), 60);
        assert_eq!(app.selected_tab, 3);

        app.history = Some(vec![history_item(200), history_item(100)]);
        app.select_tab(3);
        app.mouse(click(5, 6), 80);
        assert_eq!(app.history_index, 1);

        app.select_tab(4);
        app.mouse(click(5, 11), 80);
        assert_eq!(app.settings.selected, 2);

        app.mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 5,
                row: 10,
                modifiers: KeyModifiers::NONE,
            },
            80,
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
