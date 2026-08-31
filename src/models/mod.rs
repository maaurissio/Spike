//! Modelos normalizados y fuente-agnósticos de partidas.
#![allow(dead_code)] // Se conectan cuando MatchDetailSource entregue datos reales.

pub(crate) mod roster;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameMode {
    #[default]
    Unknown,
    Unrated,
    Competitive,
    Custom,
    Swiftplay,
    Deathmatch,
    TeamDeathmatch,
    Escalation,
}

impl GameMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "desconocido",
            Self::Unrated => "normal",
            Self::Competitive => "competitivo",
            Self::Custom => "personalizada",
            Self::Swiftplay => "swiftplay",
            Self::Deathmatch => "deathmatch",
            Self::TeamDeathmatch => "team deathmatch",
            Self::Escalation => "escalation",
        }
    }

    pub fn supports_round_timeline(self) -> bool {
        matches!(
            self,
            Self::Unrated | Self::Competitive | Self::Custom | Self::Swiftplay
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Team {
    Blue,
    Red,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundResult {
    Eliminated,
    Detonated,
    Defused,
    Surrendered,
    TimerExpired,
    Unknown,
}

impl RoundResult {
    pub fn label(self) -> &'static str {
        match self {
            Self::Eliminated => "eliminación",
            Self::Detonated => "detonada",
            Self::Defused => "desactivada",
            Self::Surrendered => "rendición",
            Self::TimerExpired => "tiempo",
            Self::Unknown => "sin dato",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundCeremony {
    Ace,
    TeamAce,
    Clutch,
    Flawless,
    Thrifty,
    Closer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerRoundStat {
    pub puuid: String,
    pub kills: u8,
    pub deaths: u8,
    pub score: Option<u32>,
    pub damage: Option<u32>,
}

impl PlayerRoundStat {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.puuid.is_empty() {
            return Err(ModelError::EmptyPlayerId);
        }
        if self.kills > 5 {
            return Err(ModelError::ImpossibleKills(self.kills));
        }
        if self.deaths > 2 {
            return Err(ModelError::ImpossibleDeaths(self.deaths));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    pub round_num: u32,
    pub winning_team: Team,
    pub round_result: RoundResult,
    pub ceremony: Option<RoundCeremony>,
    pub players: Vec<PlayerRoundStat>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchRounds {
    pub match_id: String,
    pub mode: GameMode,
    pub rounds: Vec<Round>,
}

impl MatchRounds {
    pub fn new(match_id: String, mode: GameMode, rounds: Vec<Round>) -> Result<Self, ModelError> {
        if match_id.is_empty() {
            return Err(ModelError::EmptyMatchId);
        }
        if !mode.supports_round_timeline() {
            return Err(ModelError::UnsupportedRoundMode(mode));
        }
        for (index, round) in rounds.iter().enumerate() {
            let expected = u32::try_from(index + 1).expect("el número de rondas cabe en u32");
            if round.round_num != expected {
                return Err(ModelError::NonContiguousRound {
                    expected,
                    found: round.round_num,
                });
            }
            for player in &round.players {
                player.validate()?;
            }
        }
        Ok(Self {
            match_id,
            mode,
            rounds,
        })
    }
}

/// Totales oficiales del scoreboard, no derivados de eventos de kills por ronda.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlayerMatchStats {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub combat_score: Option<u32>,
    pub damage: Option<u32>,
    pub headshots: Option<u32>,
    pub bodyshots: Option<u32>,
    pub legshots: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MatchOutcome {
    Win,
    Loss,
    Draw,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerMatch {
    pub outcome: MatchOutcome,
    pub rounds_played: u32,
    pub stats: PlayerMatchStats,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    EmptyMatchId,
    EmptyPlayerId,
    ImpossibleKills(u8),
    ImpossibleDeaths(u8),
    NonContiguousRound { expected: u32, found: u32 },
    UnsupportedRoundMode(GameMode),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMatchId => f.write_str("match_id vacío"),
            Self::EmptyPlayerId => f.write_str("puuid vacío"),
            Self::ImpossibleKills(kills) => write!(f, "kills por ronda inválidos: {kills}"),
            Self::ImpossibleDeaths(deaths) => write!(f, "deaths por ronda inválidos: {deaths}"),
            Self::NonContiguousRound { expected, found } => {
                write!(
                    f,
                    "ronda no contigua: se esperaba {expected}, llegó {found}"
                )
            }
            Self::UnsupportedRoundMode(mode) => {
                write!(f, "el modo {mode:?} no usa timeline de rondas")
            }
        }
    }
}

impl std::error::Error for ModelError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(number: u32) -> Round {
        Round {
            round_num: number,
            winning_team: Team::Blue,
            round_result: RoundResult::Eliminated,
            ceremony: None,
            players: vec![PlayerRoundStat {
                puuid: "player".into(),
                kills: 2,
                deaths: 1,
                score: Some(450),
                damage: Some(200),
            }],
        }
    }

    #[test]
    fn accepts_five_round_fixture() {
        let rounds = (1..=5).map(round).collect();
        let match_rounds = MatchRounds::new("match".into(), GameMode::Competitive, rounds);
        assert!(match_rounds.is_ok());
    }

    #[test]
    fn preserves_overtime_numbering() {
        let rounds = (1..=26).map(round).collect();
        let match_rounds = MatchRounds::new("match".into(), GameMode::Competitive, rounds).unwrap();
        assert_eq!(match_rounds.rounds[25].round_num, 26);
    }

    #[test]
    fn rejects_impossible_round_stats() {
        let mut invalid = round(1);
        invalid.players[0].deaths = 3;
        let error = MatchRounds::new("match".into(), GameMode::Unrated, vec![invalid]).unwrap_err();
        assert_eq!(error, ModelError::ImpossibleDeaths(3));
    }

    #[test]
    fn rejects_non_round_modes_and_gaps() {
        assert!(MatchRounds::new("match".into(), GameMode::Deathmatch, vec![round(1)]).is_err());
        assert!(MatchRounds::new("match".into(), GameMode::Swiftplay, vec![round(1)]).is_ok());
        assert!(MatchRounds::new("match".into(), GameMode::Custom, vec![round(2)]).is_err());
    }
}
