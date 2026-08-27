//! Métricas reproducibles calculadas sobre los modelos normalizados.
#![allow(dead_code)] // La TUI consumirá estos resultados en la fase de historial.

use crate::models::{MatchOutcome, PlayerMatch, PlayerMatchStats};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerformanceSummary {
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub kd: Option<f32>,
    pub kda: Option<f32>,
    pub win_rate: Option<f32>,
    pub hs_percent: Option<f32>,
    pub adr: Option<f32>,
    pub acs: Option<f32>,
}

pub fn summarize(matches: &[PlayerMatch]) -> PerformanceSummary {
    let mut summary = PerformanceSummary {
        matches: u32::try_from(matches.len()).expect("la cantidad de partidas cabe en u32"),
        ..PerformanceSummary::default()
    };
    let mut rounds_played = 0;
    let mut total_damage = Some(0_u32);
    let mut total_score = Some(0_u32);
    let mut total_headshots = Some(0_u32);
    let mut total_shots = Some(0_u32);

    for player_match in matches {
        match player_match.outcome {
            MatchOutcome::Win => summary.wins += 1,
            MatchOutcome::Loss => summary.losses += 1,
            MatchOutcome::Draw => summary.draws += 1,
            MatchOutcome::Unknown => {}
        }
        summary.kills += player_match.stats.kills;
        summary.deaths += player_match.stats.deaths;
        summary.assists += player_match.stats.assists;
        rounds_played += player_match.rounds_played;
        add_optional(&mut total_damage, player_match.stats.damage);
        add_optional(&mut total_score, player_match.stats.combat_score);
        add_optional(&mut total_headshots, player_match.stats.headshots);
        add_optional(&mut total_shots, shot_count(&player_match.stats));
    }

    summary.kd = ratio(summary.kills, summary.deaths);
    summary.kda = ratio(summary.kills + summary.assists, summary.deaths);
    summary.win_rate = ratio(summary.wins, summary.wins + summary.losses);
    summary.hs_percent = match (total_headshots, total_shots) {
        (Some(headshots), Some(shots)) => ratio(headshots, shots).map(|value| value * 100.0),
        _ => None,
    };
    summary.adr = total_damage.and_then(|damage| ratio(damage, rounds_played));
    summary.acs = total_score.and_then(|score| ratio(score, rounds_played));
    summary
}

fn add_optional(total: &mut Option<u32>, next: Option<u32>) {
    *total = match (*total, next) {
        (Some(total), Some(next)) => total.checked_add(next),
        _ => None,
    };
}

fn shot_count(stats: &PlayerMatchStats) -> Option<u32> {
    let headshots = stats.headshots?;
    let bodyshots = stats.bodyshots?;
    let legshots = stats.legshots?;
    headshots.checked_add(bodyshots)?.checked_add(legshots)
}

fn ratio(numerator: u32, denominator: u32) -> Option<f32> {
    (denominator != 0).then(|| numerator as f32 / denominator as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MatchOutcome, PlayerMatch, PlayerMatchStats};

    fn match_with(outcome: MatchOutcome, stats: PlayerMatchStats) -> PlayerMatch {
        PlayerMatch {
            outcome,
            rounds_played: 20,
            stats,
        }
    }

    #[test]
    fn calculates_stats_from_official_match_totals() {
        let matches = vec![
            match_with(
                MatchOutcome::Win,
                PlayerMatchStats {
                    kills: 20,
                    deaths: 10,
                    assists: 5,
                    combat_score: Some(4000),
                    damage: Some(3000),
                    headshots: Some(10),
                    bodyshots: Some(20),
                    legshots: Some(10),
                },
            ),
            match_with(
                MatchOutcome::Loss,
                PlayerMatchStats {
                    kills: 10,
                    deaths: 20,
                    assists: 5,
                    combat_score: Some(3000),
                    damage: Some(2000),
                    headshots: Some(5),
                    bodyshots: Some(10),
                    legshots: Some(5),
                },
            ),
        ];

        let result = summarize(&matches);

        assert_eq!(result.kd, Some(1.0));
        assert_eq!(result.kda, Some(4.0 / 3.0));
        assert_eq!(result.win_rate, Some(0.5));
        assert_eq!(result.hs_percent, Some(25.0));
        assert_eq!(result.adr, Some(125.0));
        assert_eq!(result.acs, Some(175.0));
    }

    #[test]
    fn omits_optional_metrics_when_a_provider_lacks_fields() {
        let result = summarize(&[match_with(
            MatchOutcome::Draw,
            PlayerMatchStats {
                kills: 1,
                deaths: 0,
                assists: 0,
                ..PlayerMatchStats::default()
            },
        )]);

        assert_eq!(result.kd, None);
        assert_eq!(result.kda, None);
        assert_eq!(result.win_rate, None);
        assert_eq!(result.hs_percent, None);
        assert_eq!(result.adr, None);
        assert_eq!(result.acs, None);
    }

    #[test]
    fn ignores_draws_for_win_rate() {
        let result = summarize(&[
            match_with(MatchOutcome::Win, PlayerMatchStats::default()),
            match_with(MatchOutcome::Draw, PlayerMatchStats::default()),
            match_with(MatchOutcome::Loss, PlayerMatchStats::default()),
        ]);
        assert_eq!(result.draws, 1);
        assert_eq!(result.win_rate, Some(0.5));
    }
}
