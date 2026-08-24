use std::{
    io::{self, Write},
    process::Command,
    time::{Instant, SystemTime},
};

use crate::{VERSION, game::Observation, watch::Watcher};

fn timestamp(time: SystemTime) -> String {
    match time.elapsed() {
        Ok(age) => format!("hace {} s", age.as_secs()),
        Err(_) => "ahora".into(),
    }
}
fn clear_terminal() {
    if cfg!(windows) {
        let _ = Command::new("cmd").args(["/C", "cls"]).status();
    } else {
        print!("\x1B[2J\x1B[H");
    }
}
pub fn draw_watch(
    watcher: &Watcher,
    observation: &Observation,
    started: Instant,
    interactive: bool,
) {
    if interactive {
        clear_terminal();
    }
    println!("VTRACKER WATCH  ·  MVP  ·  v{VERSION}\n────────────────────────────────────────");
    println!(
        "Estado          {}\nFuente          {}\nCliente         {}\nJuego           {}\nSesión          {} s\nTransiciones    {}",
        watcher.state,
        observation.source,
        if observation.client_found {
            "detectado"
        } else {
            "no detectado"
        },
        if observation.game_found {
            "en ejecución"
        } else {
            "no detectado"
        },
        started.elapsed().as_secs(),
        watcher.transitions.len()
    );
    if let Some(last) = watcher.transitions.last() {
        println!(
            "Último cambio   {} → {} ({})",
            last.from,
            last.to,
            timestamp(last.at)
        );
    }
    println!(
        "────────────────────────────────────────\nSolo se observan procesos del sistema; no se accede a memoria ni se automatiza el juego."
    );
    if interactive {
        println!("Actualización automática. Ctrl+C para salir.");
    }
    let _ = io::stdout().flush();
}
pub fn print_help() {
    println!(
        "vtracker {VERSION}\n\nUSO:\n  vtracker watch [--once] [--interval SEGUNDOS]\n  vtracker doctor\n\nVARIABLES:\n  VTRACKER_STATE=closed|idle|game  Simula un estado para pruebas."
    );
}
