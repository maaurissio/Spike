//! Punto de entrada del MVP de VTracker.

mod config;
mod diagnostics;
mod game;
mod ui;
mod watch;

use std::{env, io, thread, time::Instant};

use config::Config;
use game::detect;
use ui::{draw_watch, print_help};
use watch::{Watcher, log_transition};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "watch".into());
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        print_help();
        return;
    }
    if command == "doctor" {
        if let Some(option) = args.next() {
            eprintln!("Opción desconocida: {option}");
            std::process::exit(2);
        }
        diagnostics::doctor();
        return;
    }
    if command != "watch" {
        eprintln!("Comando no disponible en el MVP: {command}\nUsa `vtracker watch`.");
        std::process::exit(2);
    }

    let mut once = false;
    let (mut config, config_warning) = Config::effective();
    if let Some(warning) = config_warning {
        eprintln!("Advertencia: configuración ignorada ({warning}). Se usan valores por defecto.");
    }
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--once" => once = true,
            "--interval" => match args.next().and_then(|value| value.parse::<u64>().ok()) {
                Some(seconds) if (1..=60).contains(&seconds) => {
                    config.interval = std::time::Duration::from_secs(seconds)
                }
                _ => {
                    eprintln!("--interval debe estar entre 1 y 60 segundos.");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                print_help();
                return;
            }
            _ => {
                eprintln!("Opción desconocida: {arg}");
                std::process::exit(2);
            }
        }
    }

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
