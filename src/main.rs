//! Punto de entrada del MVP de VTracker.

mod analytics;
mod cache;
mod cli;
mod config;
mod diagnostics;
mod game;
mod models;
mod providers;
mod ui;
mod watch;

use std::{env, io, thread};

use cli::{Command, ConfigCommand};
use config::Config;
use providers::{
    GameStateSource, LocalClientSource, MatchDetailSource, ProcessGameStateSource,
    capabilities::GamePhase, match_detail::CompletedMatch, resolve_with_fallback,
};
use ui::{draw_watch, print_help};
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
        Command::Help => {
            print_help();
        }
        Command::Doctor => {
            diagnostics::doctor();
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

fn run_watch(config: Config, once: bool) {
    let interactive = io::IsTerminal::is_terminal(&io::stdout());
    let mut watcher = Watcher::default();
    let fallback = ProcessGameStateSource::new();
    let local = LocalClientSource::new();
    let match_details = MatchDetailSource::new();
    let mut completed_match = None;
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
            if transition.to == GamePhase::PostMatch {
                completed_match = fetch_postmatch_once(&local, &match_details);
            } else {
                completed_match = None;
            }
        }
        draw_watch(&info, completed_match.as_ref(), interactive);
        if once || !interactive {
            break;
        }
        thread::sleep(config.interval);
    }
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
