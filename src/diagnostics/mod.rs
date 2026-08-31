use std::{env, fmt::Write};

use crate::{
    VERSION,
    config::{Config, config_path},
    game::{detect, process_list},
    providers::{
        GameStateSource, LocalClientSource, ProviderError, StateInfo,
        capabilities::{CONFIDENCE_LEVELS, FINE_GRAINED_PHASES, GamePhase},
        lockfile,
    },
};

pub fn find_riot_processes(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| {
            let line = line.to_lowercase();
            line.contains("valorant") || line.contains("riotclient") || line.contains("riot client")
        })
        .take(8)
        .map(|line| line.trim_matches('"').to_string())
        .collect()
}

pub fn build_report() -> String {
    build_report_inner(env::var_os("SPIKE_STATE").is_some())
}

pub(crate) fn build_report_inner(simulation_active: bool) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "SPIKE DOCTOR  ·  v{VERSION}\n────────────────────────────────────────"
    );
    let _ = writeln!(out, "Sistema         {}", env::consts::OS);
    let _ = writeln!(
        out,
        "Detector        procesos locales (sin acceso a memoria)"
    );
    match config_path() {
        Some(path) if path.exists() => match Config::load() {
            Ok(_) => {
                let _ = writeln!(out, "Configuración   válida: {}", path.display());
            }
            Err(error) => {
                let _ = writeln!(out, "Configuración   inválida: {error}");
            }
        },
        Some(path) => {
            let _ = writeln!(
                out,
                "Configuración   no encontrada (opcional): {}",
                path.display()
            );
        }
        None => {
            let _ = writeln!(out, "Configuración   APPDATA no está disponible");
        }
    }
    let (config, _) = Config::effective();
    let _ = writeln!(out, "Intervalo       {} s", config.interval.as_secs());
    let _ = writeln!(
        out,
        "Log transiciones {}",
        if config.log_transitions {
            "activo"
        } else {
            "desactivado"
        }
    );
    write_lockfile_status(&mut out);
    write_local_source_status(&mut out);
    let _ = writeln!(
        out,
        "Fases finas     {} planificadas para proveedor local autorizado",
        FINE_GRAINED_PHASES.len()
    );
    let _ = writeln!(
        out,
        "Confianza       {} niveles soportados",
        CONFIDENCE_LEVELS.len()
    );
    let process_query_ok = match process_list() {
        Ok(processes) => {
            let matches = find_riot_processes(&processes);
            let _ = writeln!(out, "Consulta        correcta");
            if matches.is_empty() {
                let _ = writeln!(out, "Procesos Riot   no detectados");
            } else {
                let _ = writeln!(out, "Procesos Riot   detectados:");
                for process in matches {
                    let _ = writeln!(out, "  - {process}");
                }
            }
            true
        }
        Err(error) => {
            let _ = writeln!(out, "Consulta        falló: {error}");
            false
        }
    };
    if simulation_active {
        let _ = writeln!(
            out,
            "Simulación      activa mediante SPIKE_STATE (el estado mostrado no es real)"
        );
    }
    let _ = writeln!(out, "Estado actual   {}", detect().state);
    let _ = writeln!(out, "────────────────────────────────────────");
    write_detector_result(&mut out, process_query_ok);
    out
}

fn write_lockfile_status(out: &mut String) {
    match lockfile::read_default() {
        Ok(lockfile) => {
            let auth = if lockfile.has_password() {
                "presente"
            } else {
                "ausente"
            };
            let _ = writeln!(
                out,
                "Lockfile API    detectado: {} pid={} port={} protocol={} auth={auth}",
                lockfile.name, lockfile.pid, lockfile.port, lockfile.protocol
            );
        }
        Err(lockfile::LockfileError::MissingLocalAppData) => {
            let _ = writeln!(out, "Lockfile API    LOCALAPPDATA no está disponible");
        }
        Err(lockfile::LockfileError::NotFound(_)) => {
            let _ = writeln!(out, "Lockfile API    no encontrado (Riot Client cerrado)");
        }
        Err(error) => {
            let _ = writeln!(out, "Lockfile API    {error}");
        }
    }
}

fn write_local_source_status(out: &mut String) {
    let source = LocalClientSource::new();
    let result = source.fetch();
    let client_open = result
        .as_ref()
        .is_ok_and(|info| info.phase != GamePhase::ClientClosed);
    write_local_source_result(out, result);
    if client_open {
        match source.inspect_api() {
            Ok(info) => {
                let _ = writeln!(
                    out,
                    "API local      entitlements={} sesiones={} región={} locale={}",
                    if info.entitlements_available {
                        "válidos"
                    } else {
                        "ausentes"
                    },
                    if info.external_sessions_available {
                        "válidas"
                    } else {
                        "ausentes"
                    },
                    info.region.as_deref().unwrap_or("no informado"),
                    info.locale.as_deref().unwrap_or("no informado"),
                );
            }
            Err(ProviderError::EndpointUnavailable {
                endpoint,
                status: 404,
            }) => {
                let _ = writeln!(
                    out,
                    "API local      endpoint de sesión no disponible fuera de partida: {endpoint} (HTTP 404)"
                );
            }
            Err(error) => {
                let _ = writeln!(out, "API local      endpoints base no verificados: {error}");
            }
        }
        match source.validate_websocket() {
            Ok(()) => {
                let _ = writeln!(
                    out,
                    "WebSocket local handshake WAMP y suscripción validados"
                );
            }
            Err(error) => {
                let _ = writeln!(out, "WebSocket local no verificado: {error}");
            }
        }
        match source.sample_websocket_events(3) {
            Ok(events) => {
                let mut phases = events
                    .iter()
                    .filter_map(|event| event.phase_hint())
                    .map(|phase| phase.label())
                    .collect::<Vec<_>>();
                phases.sort_unstable();
                phases.dedup();
                let _ = writeln!(
                    out,
                    "WebSocket eventos {} (solo metadatos); fases observadas: {}",
                    events.len(),
                    if phases.is_empty() {
                        "ninguna en la ventana de muestra".into()
                    } else {
                        phases.join(", ")
                    }
                );
            }
            Err(error) => {
                let _ = writeln!(out, "WebSocket eventos no muestreados: {error}");
            }
        }
    }
}

fn write_local_source_result(out: &mut String, result: Result<StateInfo, ProviderError>) {
    match result {
        Ok(info) if info.phase == GamePhase::ClientClosed => {
            let _ = writeln!(out, "Proveedor local cliente cerrado (sin petición HTTP)");
        }
        Ok(info) => {
            let _ = writeln!(
                out,
                "Proveedor local correcto: fase={} detalle={} confianza={} fuente={} cliente={} juego={}",
                info.phase,
                if info.phase.is_fine_grained() {
                    "fino"
                } else {
                    "grueso"
                },
                info.confidence,
                info.source,
                if info.client_found { "sí" } else { "no" },
                if info.game_found { "sí" } else { "no" },
            );
        }
        Err(error) => {
            let _ = writeln!(out, "Proveedor local no disponible: {error}");
        }
    }
}

fn write_detector_result(out: &mut String, process_query_ok: bool) {
    if process_query_ok {
        let _ = writeln!(
            out,
            "Resultado: la detección local está lista. Las fases finas solo se muestran cuando el WebSocket local aporta una URI reciente e inequívoca; en otro caso se conserva el estado por procesos."
        );
    } else {
        let _ = writeln!(
            out,
            "Resultado: no fue posible consultar procesos. Ejecuta el comando en una consola normal y revisa sus permisos."
        );
    }
}

pub fn doctor() {
    print!("{}", build_report());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_riot_processes_case_insensitive() {
        let input = "\"RiotClientServices.exe\",\"123\",\"Console\",\"1\",\"10 K\"\n\"chrome.exe\",\"456\",\"Console\",\"1\",\"10 K\"\n\"VALORANT-Win64-Shipping.exe\",\"789\",\"Console\",\"1\",\"100 K\"";
        let found = find_riot_processes(input);
        assert_eq!(found.len(), 2);
        assert!(found[0].contains("RiotClientServices"));
        assert!(found[1].contains("VALORANT"));
    }

    #[test]
    fn finds_riot_client_with_space() {
        let input = "\"Riot Client Services\",\"123\"";
        let found = find_riot_processes(input);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn limits_to_eight_matches() {
        let input = (0..20)
            .map(|i| format!("\"valorant-{i}.exe\",\"{i}\""))
            .collect::<Vec<_>>()
            .join("\n");
        let found = find_riot_processes(&input);
        assert_eq!(found.len(), 8);
    }

    #[test]
    fn returns_empty_when_no_match() {
        let input = "\"chrome.exe\",\"123\"\n\"explorer.exe\",\"456\"";
        let found = find_riot_processes(input);
        assert!(found.is_empty());
    }

    #[test]
    fn report_contains_expected_sections() {
        let report = build_report();
        assert!(report.contains("SPIKE DOCTOR"));
        assert!(report.contains("Sistema"));
        assert!(report.contains("Detector"));
        assert!(report.contains("Intervalo"));
        assert!(report.contains("Log transiciones"));
        assert!(report.contains("Consulta"));
        assert!(report.contains("Estado actual"));
        assert!(report.contains("Resultado:"));
    }

    #[test]
    fn successful_report_result_mentions_limitations() {
        let mut report = String::new();
        write_detector_result(&mut report, true);
        assert!(report.contains("URI reciente e inequívoca"));
    }

    #[test]
    fn local_provider_result_is_human_readable() {
        let mut report = String::new();
        write_local_source_result(
            &mut report,
            Ok(StateInfo::new(
                GamePhase::Idle,
                crate::game::GameState::Idle,
                crate::providers::capabilities::Confidence::High,
                "local-client",
                true,
                false,
            )),
        );
        assert!(report.contains("Proveedor local correcto"));
        assert!(report.contains("Cliente disponible"));
        assert!(!report.contains("password"));
    }

    #[test]
    fn local_provider_closed_does_not_count_as_an_error() {
        let mut report = String::new();
        write_local_source_result(
            &mut report,
            Ok(StateInfo::new(
                GamePhase::ClientClosed,
                crate::game::GameState::ClientClosed,
                crate::providers::capabilities::Confidence::High,
                "local-client",
                false,
                false,
            )),
        );
        assert!(report.contains("cliente cerrado"));
        assert!(!report.contains("no disponible"));
    }

    #[test]
    fn report_shows_simulation_when_env_set() {
        let report = build_report_inner(true);
        assert!(report.contains("Simulación"));
        assert!(report.contains("SPIKE_STATE"));
        let report_no_sim = build_report_inner(false);
        assert!(!report_no_sim.contains("Simulación"));
    }

    #[test]
    fn build_report_respects_env_var() {
        // Verifica que build_report() lee realmente la variable de entorno.
        // Se usa serialización manual para evitar carreras con otros tests que tocan SPIKE_STATE.
        let _guard = crate::test_support::env_lock();
        let original = std::env::var_os("SPIKE_STATE");
        unsafe { std::env::set_var("SPIKE_STATE", "idle") };
        let with_sim = build_report();
        if let Some(val) = original {
            unsafe { std::env::set_var("SPIKE_STATE", val) };
        } else {
            unsafe { std::env::remove_var("SPIKE_STATE") };
        }
        let without_sim = build_report();
        assert!(with_sim.contains("Simulación"));
        assert!(!without_sim.contains("Simulación"));
    }
}
