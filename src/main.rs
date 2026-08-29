//! Punto de entrada del MVP de VTracker.

mod analytics;
mod cache;
mod cli;
mod config;
mod diagnostics;
mod game;
mod models;
mod providers;
mod tui;
mod ui;
mod watch;

use std::{env, io, thread};

use analytics::{summarize, summarize_by_category};
use cache::{L1Cache, L1CacheSettings};
use cli::{Command, ConfigCommand};
use config::Config;
use providers::{
    GameStateSource, HistorySource, LiveMatchSource, LocalClientSource, MatchDetailSource,
    PlayerProfileSource, ProcessGameStateSource,
    capabilities::GamePhase,
    live_match::LiveMatchContext,
    match_detail::CompletedMatch,
    profile::{CompetitiveProfile, OwnProfile},
    resolve_with_fallback,
};
use ui::{draw_watch, history_view, player_view_profile, print_help, stats_view};
use watch::{Watcher, log_transition};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod test_support;

fn main() {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let command = match cli::parse(&raw_args) {
        Ok(cmd) => cmd,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    match command {
        Command::Dashboard { demo } => {
            let (config, config_warning) = if demo {
                (Config::default(), None)
            } else {
                Config::effective()
            };
            if let Some(warning) = config_warning {
                eprintln!(
                    "Advertencia: configuración ignorada ({warning}). Se usan valores por defecto."
                );
            }
            if let Err(error) = tui::run(config, demo) {
                eprintln!("No se pudo abrir el dashboard: {error}");
                std::process::exit(1);
            }
        }
        Command::Help => {
            print_help();
        }
        Command::Doctor => {
            diagnostics::doctor();
        }
        Command::History(args) => {
            run_history(args.limit);
        }
        Command::Player => {
            run_player();
        }
        Command::Stats(args) => {
            run_stats(args.limit);
        }
        Command::Config(command) => {
            let result = match command {
                ConfigCommand::Show => config::show(),
                ConfigCommand::Validate => config::validate(),
                ConfigCommand::Edit(args) => config::edit(args.interval_secs, args.log_transitions),
            };
            match result {
                Ok(output) => println!("{output}"),
                Err(error) => {
                    eprintln!("Configuración inválida: {error}");
                    std::process::exit(1);
                }
            }
        }
        Command::Watch(wargs) => {
            let (mut config, config_warning) = Config::effective();
            if let Some(warning) = config_warning {
                eprintln!(
                    "Advertencia: configuración ignorada ({warning}). Se usan valores por defecto."
                );
            }
            if let Some(seconds) = wargs.interval_secs {
                config.interval = std::time::Duration::from_secs(seconds);
            }
            run_watch(config, wargs.once);
        }
    }
}

fn run_history(limit: u8) {
    let local = LocalClientSource::new();
    let source = HistorySource::new();
    match local
        .history_request(limit)
        .and_then(|request| source.fetch_own(&request))
    {
        Ok(entries) => println!("{}", history_view(&entries)),
        Err(error) => {
            eprintln!("No se pudo consultar tu historial: {error}");
            std::process::exit(1);
        }
    }
}

fn run_player() {
    let local = LocalClientSource::new();
    let source = PlayerProfileSource::new();
    match local.profile_request().and_then(|request| {
        source.fetch_own(&request).map(|profile| {
            let mut competitive = source.fetch_own_competitive(&request).ok().flatten();
            let updates = source
                .fetch_own_competitive_updates(&request, 5)
                .unwrap_or_default();
            if competitive.is_none() {
                competitive = updates
                    .first()
                    .and_then(CompetitiveProfile::from_latest_update);
            }
            (profile, competitive, updates)
        })
    }) {
        Ok((profile, competitive, updates)) => println!(
            "{}",
            player_view_profile(&profile, competitive.as_ref(), &updates)
        ),
        Err(error) => {
            eprintln!("No se pudo consultar tu perfil: {error}");
            std::process::exit(1);
        }
    }
}

fn run_stats(limit: u8) {
    let local = LocalClientSource::new();
    let history = HistorySource::new();
    let details = MatchDetailSource::new();
    let result = local.history_request(limit).and_then(|request| {
        let matches = history.fetch_own_matches(&request)?;
        let totals = matches
            .into_iter()
            .filter_map(|entry| {
                details
                    .fetch_own_totals(&request.match_detail_request(entry.match_id))
                    .ok()
                    .map(|totals| (entry.entry.queue, totals.map, totals.agent, totals.stats))
            })
            .collect::<Vec<_>>();
        if totals.is_empty() {
            Err(providers::ProviderError::Unavailable(
                "no hubo detalles de partidas disponibles para calcular estadísticas".into(),
            ))
        } else {
            Ok(totals)
        }
    });
    match result {
        Ok(matches) => {
            let player_matches = matches
                .iter()
                .map(|(_, _, _, player_match)| player_match.clone())
                .collect::<Vec<_>>();
            let by_mode = matches
                .iter()
                .map(|(mode, _, _, player_match)| (mode.clone(), player_match.clone()))
                .collect::<Vec<_>>();
            let by_map = matches
                .iter()
                .map(|(_, map, _, player_match)| (map.clone(), player_match.clone()))
                .collect::<Vec<_>>();
            let by_agent = matches
                .iter()
                .map(|(_, _, agent, player_match)| (agent.clone(), player_match.clone()))
                .collect::<Vec<_>>();
            println!(
                "{}",
                stats_view(
                    &summarize(&player_matches),
                    &summarize_by_category(&by_mode),
                    &summarize_by_category(&by_map),
                    &summarize_by_category(&by_agent),
                )
            )
        }
        Err(error) => {
            eprintln!("No se pudieron calcular tus estadísticas: {error}");
            std::process::exit(1);
        }
    }
}

fn run_watch(config: Config, once: bool) {
    let interactive = io::IsTerminal::is_terminal(&io::stdout());
    let mut watcher = Watcher::default();
    let fallback = ProcessGameStateSource::new();
    let local = LocalClientSource::new();
    let match_details = MatchDetailSource::new();
    let live_match_source = LiveMatchSource::new();
    let profile_source = PlayerProfileSource::new();
    let profile_cache = L1Cache::new(L1CacheSettings {
        capacity: 1,
        ttl: std::time::Duration::from_secs(60),
    });
    let mut completed_match = None;
    let mut live_match = None;
    let mut own_profile = None;
    local.start_event_listener();
    loop {
        debug_assert!(fallback.is_available());
        let info = match resolve_with_fallback(&local, &fallback) {
            Ok(info) => info,
            Err(error) => {
                eprintln!("Advertencia: proveedores locales fallaron: {error}");
                providers::StateInfo::unknown(fallback.name())
            }
        };
        let transition = watcher.observe(&info);
        if let Some(transition) = transition.as_ref() {
            if config.log_transitions
                && let Err(error) = log_transition(transition)
            {
                eprintln!("Advertencia: no se pudo guardar el log: {error}");
            }
            if transition.to == GamePhase::InMatch {
                live_match = fetch_live_match_once(&local, &live_match_source);
                completed_match = None;
                own_profile = None;
            } else if transition.to == GamePhase::PostMatch {
                completed_match = fetch_postmatch_once(&local, &match_details);
                live_match = None;
                own_profile = None;
            } else if transition.to == GamePhase::Idle {
                if let Some(profile) = fetch_profile_once(&local, &profile_source) {
                    profile_cache.insert("own-profile", profile);
                }
                own_profile = profile_cache.get("own-profile").as_deref().cloned();
            } else {
                completed_match = None;
                live_match = None;
                own_profile = None;
            }
        }
        draw_watch(
            &info,
            live_match.as_ref(),
            own_profile.as_ref(),
            completed_match.as_ref(),
            interactive,
        );
        if once || !interactive {
            break;
        }
        thread::sleep(config.interval);
    }
}

/// Se invoca solo al entrar a cliente disponible. La caché conserva durante un
/// minuto el último perfil propio si una consulta posterior falla.
fn fetch_profile_once(
    local: &LocalClientSource,
    source: &PlayerProfileSource,
) -> Option<OwnProfile> {
    local
        .profile_request()
        .and_then(|request| source.fetch_own(&request))
        .ok()
}

fn fetch_live_match_once(
    local: &LocalClientSource,
    source: &LiveMatchSource,
) -> Option<LiveMatchContext> {
    local
        .live_match_request()
        .and_then(|request| source.fetch(&request))
        .ok()
}

/// Tras una transición local confirmada a postpartida, realiza como máximo una
/// consulta GET. Los fallos no revelan URL, IDs ni tokens en la vista de jugador.
fn fetch_postmatch_once(
    local: &LocalClientSource,
    source: &MatchDetailSource,
) -> Option<CompletedMatch> {
    local
        .match_detail_request()
        .and_then(|request| source.fetch_completed(&request))
        .ok()
}
