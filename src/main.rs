//! Punto de entrada del MVP de VTracker.

mod cli;
mod config;
mod diagnostics;
mod game;
mod providers;
mod ui;
mod watch;

use std::{env, io, thread, time::Instant};

use cli::Command;
use config::Config;
use providers::{GameStateSource, ProcessGameStateSource};
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
    let started = Instant::now();
    let mut watcher = Watcher::default();
    let source = ProcessGameStateSource::new();
    loop {
        debug_assert!(
            source.is_available(),
            "{} provider should be available",
            source.name()
        );
        let info = match source.fetch() {
            Ok(info) => info,
            Err(error) => {
                eprintln!("Advertencia: proveedor {} falló: {error}", source.name());
                providers::StateInfo::unknown(source.name())
            }
        };
        let observation = info.observation();
        let transition = watcher.observe(observation.clone());
        if config.log_transitions
            && let Some(transition) = transition
            && let Err(error) = log_transition(&transition)
        {
            eprintln!("Advertencia: no se pudo guardar el log: {error}");
        }
        draw_watch(&watcher, &info, started, interactive);
        if once || !interactive {
            break;
        }
        thread::sleep(config.interval);
    }
}
