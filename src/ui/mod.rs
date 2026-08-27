use std::{
    io::{self, Write},
    process::Command,
};

use crate::{
    VERSION,
    providers::{StateInfo, match_detail::CompletedMatch},
};

fn clear_terminal() {
    if cfg!(windows) {
        let _ = Command::new("cmd").args(["/C", "cls"]).status();
    } else {
        print!("\x1B[2J\x1B[H");
    }
}
pub fn draw_watch(info: &StateInfo, completed_match: Option<&CompletedMatch>, interactive: bool) {
    if interactive {
        clear_terminal();
    }
    print!("{}", player_view(info, completed_match));
    if interactive {
        println!("Actualización automática. Ctrl+C para salir.");
    }
    let _ = io::stdout().flush();
}

fn player_view(info: &StateInfo, completed_match: Option<&CompletedMatch>) -> String {
    let mut view = format!(
        "VTRACKER  ·  v{VERSION}\n────────────────────────────────────────\nEstado          {}\n────────────────────────────────────────\n",
        info.phase
    );
    if let Some(completed_match) = completed_match {
        let rows = completed_match
            .rounds
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
                completed_match.rounds.mode.label(),
                completed_match.rounds.rounds.len(),
            ));
        }
    }
    view
}
pub fn print_help() {
    println!(
        "vtracker {VERSION}\n\nUSO:\n  vtracker watch [--once] [--interval SEGUNDOS]\n  vtracker doctor\n  vtracker config show|validate\n  vtracker config edit [--interval SEGUNDOS] [--log-transitions true|false]\n\nVARIABLES:\n  VTRACKER_STATE=closed|idle|game  Simula un estado para pruebas."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::GameState,
        models::{GameMode, MatchRounds, PlayerRoundStat, Round, RoundResult, Team},
        providers::capabilities::{Confidence, GamePhase},
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

        let view = player_view(&info, None);

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
            rounds: MatchRounds::new(
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
        };

        let view = player_view(&info, Some(&completed_match));

        assert!(view.contains("Última partida  competitivo"));
        assert!(view.contains("eliminación  2  1"));
        assert!(!view.contains("other"));
    }
}
