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
use game::detect;
use ui::{draw_watch, print_help};
use watch::{Watcher, log_transition};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            return;
        }
        Command::Doctor => {
            diagnostics::doctor();
            return;
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
            return;
        }
    }
}

fn run_watch(config: Config, once: bool) {
    let interactive = io::IsTerminal::is_terminal(&io::stdout());
    let started = Instant::now();
    let mut watcher = Watcher::default();
    loop {
        let observation = detect();
        let transition = watcher.observe(observation.clone());
        if config.log_transitions
            && let Some(transition) = transition
            && let Err(error) = log_transition(&transition)
        {
            eprintln!("Advertencia: no se pudo guardar el log: {error}");
        }
        draw_watch(&watcher, &observation, started, interactive);
        if once || !interactive {
            break;
        }
        thread::sleep(config.interval);
    }
}
