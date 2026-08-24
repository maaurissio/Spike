use std::env;

use crate::{
    VERSION,
    config::{Config, config_path},
    game::{detect, process_list},
};

pub fn doctor() {
    println!("VTRACKER DOCTOR  ·  v{VERSION}\n────────────────────────────────────────");
    println!("Sistema         {}", env::consts::OS);
    println!("Detector        procesos locales (sin acceso a memoria)");
    match config_path() {
        Some(path) if path.exists() => match Config::load() {
            Ok(_) => println!("Configuración   válida: {}", path.display()),
            Err(error) => println!("Configuración   inválida: {error}"),
        },
        Some(path) => println!(
            "Configuración   no encontrada (opcional): {}",
            path.display()
        ),
        None => println!("Configuración   APPDATA no está disponible"),
    }
    let (config, _) = Config::effective();
    println!("Intervalo       {} s", config.interval.as_secs());
    println!(
        "Log transiciones {}",
        if config.log_transitions {
            "activo"
        } else {
            "desactivado"
        }
    );
    let process_query_ok = match process_list() {
        Ok(processes) => {
            let matches: Vec<_> = processes
                .lines()
                .filter(|line| {
                    let line = line.to_lowercase();
                    line.contains("valorant")
                        || line.contains("riotclient")
                        || line.contains("riot client")
                })
                .take(8)
                .collect();
            println!("Consulta        correcta");
            if matches.is_empty() {
                println!("Procesos Riot   no detectados");
            } else {
                println!("Procesos Riot   detectados:");
                for process in matches {
                    println!("  - {}", process.trim_matches('"'));
                }
            }
            true
        }
        Err(error) => {
            println!("Consulta        falló: {error}");
            false
        }
    };
    if env::var_os("VTRACKER_STATE").is_some() {
        println!("Simulación      activa mediante VTRACKER_STATE (el estado mostrado no es real)");
    }
    println!("Estado actual   {}", detect().state);
    println!("────────────────────────────────────────");
    if process_query_ok {
        println!(
            "Resultado: el detector de procesos está listo. No puede distinguir lobby, selección o partida real; esa capacidad requiere una fuente autorizada adicional."
        );
    } else {
        println!(
            "Resultado: no fue posible consultar procesos. Ejecuta el comando en una consola normal y revisa sus permisos."
        );
    }
}
