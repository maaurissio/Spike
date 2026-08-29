use std::{
    io::{self, Write},
    process::Command,
};

use crate::{
    VERSION,
    providers::{StateInfo, live_match::LiveMatchContext, match_detail::CompletedMatch},
};

fn clear_terminal() {
    if cfg!(windows) {
        let _ = Command::new("cmd").args(["/C", "cls"]).status();
    } else {
        print!("\x1B[2J\x1B[H");
    }
}
pub fn draw_watch(
    info: &StateInfo,
    live_match: Option<&LiveMatchContext>,
    own_profile: Option<&crate::providers::profile::OwnProfile>,
    completed_match: Option<&CompletedMatch>,
    interactive: bool,
) {
    if interactive {
        clear_terminal();
    }
    print!(
        "{}",
        player_view(info, live_match, own_profile, completed_match)
    );
    if interactive {
        println!("Actualización automática. Ctrl+C para salir.");
    }
    let _ = io::stdout().flush();
}

fn player_view(
    info: &StateInfo,
    live_match: Option<&LiveMatchContext>,
    own_profile: Option<&crate::providers::profile::OwnProfile>,
    completed_match: Option<&CompletedMatch>,
) -> String {
    let mut view = format!(
        "VTRACKER  ·  v{VERSION}\n────────────────────────────────────────\nEstado          {}\n────────────────────────────────────────\n",
        info.phase
    );
    if let Some(live_match) = live_match {
        view.push_str(&format!("Modo            {}\nMapa            {}\nAgente          {}\n────────────────────────────────────────\n", live_match.mode, live_match.map, live_match.agent.as_deref().unwrap_or("no disponible")));
    }
    if let Some(profile) = own_profile {
        view.push_str(&format!(
            "Nivel de cuenta {}\nExperiencia      {} XP\n────────────────────────────────────────\n",
            profile.level, profile.xp
        ));
    }
    if let Some(completed_match) = completed_match {
        if let Some(rounds) = completed_match.rounds.as_ref() {
            let rows = rounds
                .rounds
                .iter()
                .filter_map(|round| {
                    round
                        .players
                        .iter()
                        .find(|player| player.puuid == completed_match.own_puuid)
                        .map(|player| {
                            format!(
                                "{:>2}  {:<12} {:>1}  {:>1}\n",
                                round.round_num,
                                round.round_result.label(),
                                player.kills,
                                player.deaths
                            )
                        })
                })
                .collect::<String>();
            if !rows.is_empty() {
                view.push_str(&format!(
                    "Última partida  {} · {} rondas\nRonda Resultado       K  D\n{rows}────────────────────────────────────────\n",
                    rounds.mode.label(),
                    rounds.rounds.len(),
                ));
            }
        } else if let Some(summary) = completed_match.summary.as_ref() {
            view.push_str(&format!(
                "Última partida  {}\nK  D  A  Puntos\n{}  {}  {}  {}\n────────────────────────────────────────\n",
                summary.mode.label(),
                summary.stats.kills,
                summary.stats.deaths,
                summary.stats.assists,
                summary.stats.combat_score.unwrap_or(0),
            ));
        }
    }
    view
}
pub fn print_help() {
    println!(
        "vtracker {VERSION}\n\nUSO:\n  vtracker                           # interfaz principal\n  vtracker dashboard [--demo]        # interfaz explícita; demo ficticia sin conexión\n  vtracker watch [--once] [--interval SEGUNDOS]\n  vtracker player\n  vtracker history [--limit 1..20]\n  vtracker stats [--limit 1..5]\n  vtracker doctor\n  vtracker config show|validate\n  vtracker config edit [--interval SEGUNDOS] [--log-transitions true|false]\n\nVARIABLES:\n  VTRACKER_STATE=closed|idle|game  Simula un estado para pruebas."
    );
}

pub fn stats_view(
    summary: &crate::analytics::PerformanceSummary,
    by_mode: &[crate::analytics::CategorySummary],
    by_map: &[crate::analytics::CategorySummary],
    by_agent: &[crate::analytics::CategorySummary],
) -> String {
    let mut view = format!(
        "VTRACKER · Estadísticas propias\n────────────────────────────────────────\nPartidas        {}\nVictorias       {}\nDerrotas        {}\nK/D             {}\nKDA             {}\nWin rate        {}\n────────────────────────────────────────",
        summary.matches,
        summary.wins,
        summary.losses,
        metric(summary.kd),
        metric(summary.kda),
        summary
            .win_rate
            .map(|value| format!("{:.1}%", value * 100.0))
            .unwrap_or_else(|| "sin dato".into()),
    );
    for (title, categories) in [
        ("Por modo", by_mode),
        ("Por mapa", by_map),
        ("Por agente", by_agent),
    ] {
        if !categories.is_empty() {
            view.push_str(&format!("\n{title}\n"));
            for category in categories {
                view.push_str(&format!(
                    "{:<16} {} partidas · K/D {}\n",
                    category.label,
                    category.summary.matches,
                    metric(category.summary.kd),
                ));
            }
        }
    }
    if !by_mode.is_empty() || !by_map.is_empty() || !by_agent.is_empty() {
        view.push_str("────────────────────────────────────────");
    }
    view
}

fn metric(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "sin dato".into())
}

pub fn history_view(entries: &[crate::providers::history::HistoryEntry]) -> String {
    let mut view =
        String::from("VTRACKER · Historial propio\n────────────────────────────────────────\n");
    if entries.is_empty() {
        view.push_str("No hay partidas recientes disponibles.\n");
    } else {
        view.push_str("Modo                Cuándo\n");
        for entry in entries {
            view.push_str(&format!(
                "{:<19} {}\n",
                entry.queue,
                relative_time(entry.started_at_ms)
            ));
        }
    }
    view.push_str("────────────────────────────────────────");
    view
}

pub fn player_view_profile(
    profile: &crate::providers::profile::OwnProfile,
    competitive: Option<&crate::providers::profile::CompetitiveProfile>,
    updates: &[crate::providers::profile::CompetitiveUpdate],
) -> String {
    let mut view = format!(
        "VTRACKER · Perfil propio\n────────────────────────────────────────\nNivel de cuenta {}\nExperiencia      {} XP\n",
        profile.level, profile.xp
    );
    if let Some(competitive) = competitive {
        view.push_str(&format!(
            "Rango           {} · {} RR\n",
            competitive_tier_label(competitive.tier),
            competitive.ranked_rating,
        ));
        if competitive.games > 0 {
            view.push_str(&format!(
                "Competitivo     {}/{} victorias\n",
                competitive.wins, competitive.games,
            ));
        }
    }
    if !updates.is_empty() {
        view.push_str("Cambios RR      ");
        for (index, update) in updates.iter().enumerate() {
            if index != 0 {
                view.push_str(" · ");
            }
            view.push_str(&format!("{:+}", update.rr_earned));
            if update.performance_bonus > 0 {
                view.push_str(&format!(" (+{} bono)", update.performance_bonus));
            }
        }
        view.push('\n');
    }
    view.push_str("────────────────────────────────────────");
    view
}

pub(crate) fn competitive_tier_label(tier: u32) -> String {
    const NAMES: [&str; 23] = [
        "hierro 1",
        "hierro 2",
        "hierro 3",
        "bronce 1",
        "bronce 2",
        "bronce 3",
        "plata 1",
        "plata 2",
        "plata 3",
        "oro 1",
        "oro 2",
        "oro 3",
        "platino 1",
        "platino 2",
        "platino 3",
        "diamante 1",
        "diamante 2",
        "diamante 3",
        "ascendente 1",
        "ascendente 2",
        "ascendente 3",
        "inmortal 1",
        "inmortal 2",
    ];
    if tier == 25 {
        "radiante".into()
    } else if tier == 24 {
        "inmortal 3".into()
    } else if let Some(name) = tier
        .checked_sub(3)
        .and_then(|index| NAMES.get(index as usize))
    {
        (*name).into()
    } else {
        format!("tier {tier}")
    }
}

fn relative_time(started_at_ms: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let elapsed_secs = now.saturating_sub(started_at_ms) / 1_000;
    match elapsed_secs {
        0..=59 => "ahora".into(),
        60..=3_599 => format!("hace {} min", elapsed_secs / 60),
        3_600..=86_399 => format!("hace {} h", elapsed_secs / 3_600),
        _ => format!("hace {} d", elapsed_secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::GameState,
        models::{
            GameMode, MatchOutcome, MatchRounds, PlayerMatch, PlayerMatchStats, PlayerRoundStat,
            Round, RoundResult, Team,
        },
        providers::capabilities::{Confidence, GamePhase},
        providers::live_match::LiveMatchContext,
        providers::match_detail::CompletedMatch,
    };

    #[test]
    fn player_view_hides_diagnostics() {
        let info = StateInfo::new(
            GamePhase::InMatch,
            GameState::GameOpen,
            Confidence::High,
            "local-websocket",
            true,
            true,
        );

        let view = player_view(&info, None, None, None);

        assert!(view.contains("En partida"));
        assert!(!view.contains("Confianza"));
        assert!(!view.contains("Fuente"));
        assert!(!view.contains("Transiciones"));
    }

    #[test]
    fn player_view_shows_only_own_postmatch_rounds() {
        let info = StateInfo::new(
            GamePhase::PostMatch,
            GameState::Idle,
            Confidence::High,
            "local-websocket",
            true,
            false,
        );
        let completed_match = CompletedMatch {
            own_puuid: "me".into(),
            rounds: Some(
                MatchRounds::new(
                    "match".into(),
                    GameMode::Competitive,
                    vec![Round {
                        round_num: 1,
                        winning_team: Team::Blue,
                        round_result: RoundResult::Eliminated,
                        ceremony: None,
                        players: vec![
                            PlayerRoundStat {
                                puuid: "me".into(),
                                kills: 2,
                                deaths: 1,
                                score: None,
                                damage: None,
                            },
                            PlayerRoundStat {
                                puuid: "other".into(),
                                kills: 3,
                                deaths: 0,
                                score: None,
                                damage: None,
                            },
                        ],
                    }],
                )
                .unwrap(),
            ),
            summary: None,
            totals: crate::providers::match_detail::OwnMatchTotals {
                stats: PlayerMatch {
                    outcome: MatchOutcome::Win,
                    rounds_played: 1,
                    stats: PlayerMatchStats {
                        kills: 2,
                        deaths: 1,
                        ..Default::default()
                    },
                },
                map: "Ascent".into(),
                agent: "Sova".into(),
                own_score: Some(13),
                opponent_score: Some(9),
            },
        };

        let view = player_view(&info, None, None, Some(&completed_match));

        assert!(view.contains("Última partida  competitivo"));
        assert!(view.contains("eliminación  2  1"));
        assert!(!view.contains("other"));
    }

    #[test]
    fn player_view_shows_own_deathmatch_summary() {
        let info = StateInfo::new(
            GamePhase::PostMatch,
            GameState::Idle,
            Confidence::High,
            "local-websocket",
            true,
            false,
        );
        let completed_match = CompletedMatch {
            own_puuid: "me".into(),
            rounds: None,
            summary: Some(crate::providers::match_detail::MatchSummary {
                mode: GameMode::Deathmatch,
                stats: PlayerMatchStats {
                    kills: 25,
                    deaths: 20,
                    assists: 4,
                    combat_score: Some(2500),
                    ..Default::default()
                },
            }),
            totals: crate::providers::match_detail::OwnMatchTotals {
                stats: PlayerMatch {
                    outcome: MatchOutcome::Unknown,
                    rounds_played: 0,
                    stats: PlayerMatchStats {
                        kills: 25,
                        deaths: 20,
                        assists: 4,
                        combat_score: Some(2500),
                        ..Default::default()
                    },
                },
                map: "Ascent".into(),
                agent: "Sova".into(),
                own_score: None,
                opponent_score: None,
            },
        };

        let view = player_view(&info, None, None, Some(&completed_match));

        assert!(view.contains("Última partida  deathmatch"));
        assert!(view.contains("25  20  4  2500"));
        assert!(!view.contains("me"));
    }

    #[test]
    fn player_view_shows_only_own_live_context() {
        let info = StateInfo::new(
            GamePhase::InMatch,
            GameState::GameOpen,
            Confidence::High,
            "local-websocket",
            true,
            true,
        );
        let context = LiveMatchContext {
            mode: "deathmatch".into(),
            map: "Ascent".into(),
            agent: Some("Omen".into()),
            roster: None,
        };

        let view = player_view(&info, Some(&context), None, None);

        assert!(view.contains("Modo            deathmatch"));
        assert!(view.contains("Mapa            Ascent"));
        assert!(view.contains("Agente          Omen"));
        assert!(!view.contains("local-websocket"));
    }

    #[test]
    fn history_view_shows_no_match_ids_or_player_ids() {
        let view = history_view(&[
            crate::providers::history::HistoryEntry {
                queue: "competitivo".into(),
                started_at_ms: 0,
            },
            crate::providers::history::HistoryEntry {
                queue: "deathmatch".into(),
                started_at_ms: 0,
            },
        ]);

        assert!(view.contains("competitivo"));
        assert!(view.contains("deathmatch"));
        assert!(!view.contains("MatchID"));
        assert!(!view.contains("puuid"));
    }

    #[test]
    fn player_profile_view_shows_own_level_xp_and_competitive_snapshot() {
        let view = player_view_profile(
            &crate::providers::profile::OwnProfile {
                level: 10,
                xp: 2_500,
            },
            Some(&crate::providers::profile::CompetitiveProfile {
                tier: 18,
                ranked_rating: 50,
                wins: 20,
                games: 35,
            }),
            &[crate::providers::profile::CompetitiveUpdate {
                tier_after: 18,
                ranked_rating_after: Some(50),
                rr_earned: 20,
                performance_bonus: 3,
            }],
        );

        assert!(view.contains("Nivel de cuenta 10"));
        assert!(view.contains("Experiencia      2500 XP"));
        assert!(view.contains("Rango           diamante 1 · 50 RR"));
        assert!(view.contains("Cambios RR      +20 (+3 bono)"));
        assert!(!view.contains("puuid"));
    }

    #[test]
    fn player_view_shows_cached_own_profile_without_diagnostics() {
        let info = StateInfo::new(
            GamePhase::Idle,
            GameState::Idle,
            Confidence::High,
            "local-client",
            true,
            false,
        );
        let profile = crate::providers::profile::OwnProfile {
            level: 10,
            xp: 2_500,
        };

        let view = player_view(&info, None, Some(&profile), None);

        assert!(view.contains("Nivel de cuenta 10"));
        assert!(view.contains("Experiencia      2500 XP"));
        assert!(!view.contains("local-client"));
    }

    #[test]
    fn stats_view_shows_only_aggregate_metrics() {
        let view = stats_view(
            &crate::analytics::PerformanceSummary {
                matches: 3,
                wins: 2,
                losses: 1,
                kd: Some(1.5),
                kda: Some(2.0),
                win_rate: Some(2.0 / 3.0),
                ..Default::default()
            },
            &[crate::analytics::CategorySummary {
                label: "competitivo".into(),
                summary: crate::analytics::PerformanceSummary {
                    matches: 3,
                    kd: Some(1.5),
                    ..Default::default()
                },
            }],
            &[crate::analytics::CategorySummary {
                label: "Ascent".into(),
                summary: crate::analytics::PerformanceSummary {
                    matches: 3,
                    kd: Some(1.5),
                    ..Default::default()
                },
            }],
            &[crate::analytics::CategorySummary {
                label: "Omen".into(),
                summary: crate::analytics::PerformanceSummary {
                    matches: 3,
                    kd: Some(1.5),
                    ..Default::default()
                },
            }],
        );

        assert!(view.contains("Partidas        3"));
        assert!(view.contains("K/D             1.50"));
        assert!(view.contains("Win rate        66.7%"));
        assert!(view.contains("competitivo      3 partidas · K/D 1.50"));
        assert!(view.contains("Ascent           3 partidas · K/D 1.50"));
        assert!(view.contains("Omen             3 partidas · K/D 1.50"));
        assert!(!view.contains("MatchID"));
    }
}
