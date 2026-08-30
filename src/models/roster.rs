//! Contrato normalizado del roster.
//!
//! No contiene PUUID, Riot ID, match ID ni credenciales. Los proveedores deben
//! descartar esos identificadores antes de construir este modelo.

use std::{collections::HashSet, fmt};

use super::MatchOutcome;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DataAvailability<T> {
    Available(T),
    Hidden,
    NotAvailable,
    ApprovalRequired,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RosterSide {
    Ally,
    Enemy,
    Participant,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HistoricalStats {
    pub matches: u32,
    pub competitive_tier: Option<u64>,
    /// Último nivel positivo observado en el historial Ranked enriquecido.
    pub account_level: Option<u32>,
    pub decided_matches: u32,
    pub wins: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub headshots: u32,
    pub bodyshots: u32,
    pub legshots: u32,
    pub kast_rounds: u32,
    pub rounds_played: u32,
    pub recent: Vec<MatchOutcome>,
}

impl HistoricalStats {
    pub fn kd_hundredths(&self) -> Option<u32> {
        ratio(self.kills, self.deaths, 100)
    }

    pub fn win_rate_tenths(&self) -> Option<u32> {
        ratio(self.wins, self.decided_matches, 1_000)
    }

    pub fn headshot_rate_tenths(&self) -> Option<u32> {
        ratio(
            self.headshots,
            self.headshots + self.bodyshots + self.legshots,
            1_000,
        )
    }

    pub fn kast_rate_tenths(&self) -> Option<u32> {
        ratio(self.kast_rounds, self.rounds_played, 1_000)
    }

    fn validate(&self) -> Result<(), RosterModelError> {
        if self.wins > self.matches {
            return Err(RosterModelError::WinsExceedMatches {
                wins: self.wins,
                matches: self.matches,
            });
        }
        if self.decided_matches > self.matches || self.wins > self.decided_matches {
            return Err(RosterModelError::InvalidDecidedMatches {
                decided: self.decided_matches,
                wins: self.wins,
                matches: self.matches,
            });
        }
        if self.recent.len() > 5 {
            return Err(RosterModelError::TooManyRecentMatches(self.recent.len()));
        }
        if self.kast_rounds > self.rounds_played {
            return Err(RosterModelError::KastRoundsExceedRounds {
                kast_rounds: self.kast_rounds,
                rounds_played: self.rounds_played,
            });
        }
        Ok(())
    }
}

fn ratio(numerator: u32, denominator: u32, scale: u32) -> Option<u32> {
    if denominator == 0 {
        return None;
    }
    Some(numerator.saturating_mul(scale) / denominator)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RosterPlayer {
    pub side: RosterSide,
    pub slot: u8,
    pub is_self: bool,
    pub identity: DataAvailability<String>,
    pub agent: DataAvailability<String>,
    pub rank: DataAvailability<String>,
    pub level: DataAvailability<u32>,
    /// Etiqueta normalizada (`Grupo A`, `Grupo B`, `Solo`), nunca PartyID.
    pub premade: DataAvailability<String>,
    pub stats: DataAvailability<HistoricalStats>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RosterSnapshot {
    pub players: Vec<RosterPlayer>,
}

impl RosterSnapshot {
    pub fn new(players: Vec<RosterPlayer>) -> Result<Self, RosterModelError> {
        let mut slots = HashSet::new();
        let mut self_count = 0;

        for player in &players {
            if player.slot == 0 {
                return Err(RosterModelError::InvalidSlot(player.slot));
            }
            if !slots.insert((player.side, player.slot)) {
                return Err(RosterModelError::DuplicateSlot {
                    side: player.side,
                    slot: player.slot,
                });
            }
            if player.is_self {
                self_count += 1;
                if player.side == RosterSide::Enemy {
                    return Err(RosterModelError::SelfOnEnemySide);
                }
            }
            if let DataAvailability::Available(stats) = &player.stats {
                stats.validate()?;
            }
        }

        if self_count != 1 {
            return Err(RosterModelError::InvalidSelfCount(self_count));
        }

        Ok(Self { players })
    }

    pub fn allies(&self) -> impl Iterator<Item = &RosterPlayer> {
        self.players
            .iter()
            .filter(|player| player.side == RosterSide::Ally)
    }

    pub fn enemies(&self) -> impl Iterator<Item = &RosterPlayer> {
        self.players
            .iter()
            .filter(|player| player.side == RosterSide::Enemy)
    }

    pub fn participants(&self) -> impl Iterator<Item = &RosterPlayer> {
        self.players
            .iter()
            .filter(|player| player.side == RosterSide::Participant)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RosterModelError {
    InvalidSlot(u8),
    DuplicateSlot {
        side: RosterSide,
        slot: u8,
    },
    InvalidSelfCount(usize),
    SelfOnEnemySide,
    WinsExceedMatches {
        wins: u32,
        matches: u32,
    },
    InvalidDecidedMatches {
        decided: u32,
        wins: u32,
        matches: u32,
    },
    TooManyRecentMatches(usize),
    KastRoundsExceedRounds {
        kast_rounds: u32,
        rounds_played: u32,
    },
}

impl fmt::Display for RosterModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlot(slot) => write!(f, "slot de roster inválido: {slot}"),
            Self::DuplicateSlot { side, slot } => {
                write!(f, "slot de roster duplicado: {side:?} {slot}")
            }
            Self::InvalidSelfCount(count) => {
                write!(
                    f,
                    "el roster debe contener un jugador propio; recibió {count}"
                )
            }
            Self::SelfOnEnemySide => f.write_str("el jugador propio no puede estar en enemigos"),
            Self::WinsExceedMatches { wins, matches } => {
                write!(f, "victorias {wins} mayores que partidas {matches}")
            }
            Self::InvalidDecidedMatches {
                decided,
                wins,
                matches,
            } => write!(
                f,
                "partidas decididas inválidas: {decided}, victorias {wins}, partidas {matches}"
            ),
            Self::TooManyRecentMatches(count) => {
                write!(f, "demasiados resultados recientes: {count}")
            }
            Self::KastRoundsExceedRounds {
                kast_rounds,
                rounds_played,
            } => write!(
                f,
                "rondas KAST {kast_rounds} mayores que rondas jugadas {rounds_played}"
            ),
        }
    }
}

impl std::error::Error for RosterModelError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(side: RosterSide, slot: u8, is_self: bool) -> RosterPlayer {
        RosterPlayer {
            side,
            slot,
            is_self,
            identity: if is_self {
                DataAvailability::Available("Tú".into())
            } else {
                DataAvailability::ApprovalRequired
            },
            agent: DataAvailability::NotAvailable,
            rank: DataAvailability::ApprovalRequired,
            level: DataAvailability::NotAvailable,
            premade: DataAvailability::NotAvailable,
            stats: DataAvailability::ApprovalRequired,
        }
    }

    #[test]
    fn accepts_anonymous_five_vs_five_without_identifiers() {
        let mut players = (1..=5)
            .map(|slot| player(RosterSide::Ally, slot, slot == 1))
            .collect::<Vec<_>>();
        players.extend((1..=5).map(|slot| player(RosterSide::Enemy, slot, false)));

        let roster = RosterSnapshot::new(players).unwrap();
        assert_eq!(roster.allies().count(), 5);
        assert_eq!(roster.enemies().count(), 5);
        assert!(!format!("{roster:?}").contains("puuid"));
    }

    #[test]
    fn accepts_free_for_all_participants() {
        let players = (1..=12)
            .map(|slot| player(RosterSide::Participant, slot, slot == 1))
            .collect::<Vec<_>>();
        let roster = RosterSnapshot::new(players).unwrap();
        assert_eq!(roster.participants().count(), 12);
    }

    #[test]
    fn represents_hidden_and_unavailable_fields_explicitly() {
        let mut own = player(RosterSide::Ally, 1, true);
        own.rank = DataAvailability::NotAvailable;
        let mut hidden = player(RosterSide::Enemy, 1, false);
        hidden.identity = DataAvailability::Hidden;
        let roster = RosterSnapshot::new(vec![own, hidden]).unwrap();

        assert_eq!(roster.players[1].identity, DataAvailability::Hidden);
        assert_eq!(roster.players[0].rank, DataAvailability::NotAvailable);
    }

    #[test]
    fn rejects_duplicate_slots_and_invalid_self_placement() {
        let duplicate = vec![
            player(RosterSide::Ally, 1, true),
            player(RosterSide::Ally, 1, false),
        ];
        assert!(matches!(
            RosterSnapshot::new(duplicate),
            Err(RosterModelError::DuplicateSlot { .. })
        ));

        assert_eq!(
            RosterSnapshot::new(vec![player(RosterSide::Enemy, 1, true)]),
            Err(RosterModelError::SelfOnEnemySide)
        );
    }

    #[test]
    fn rejects_impossible_historical_totals() {
        let mut own = player(RosterSide::Ally, 1, true);
        own.stats = DataAvailability::Available(HistoricalStats {
            matches: 3,
            wins: 4,
            kills: 20,
            deaths: 10,
            ..Default::default()
        });
        assert!(matches!(
            RosterSnapshot::new(vec![own]),
            Err(RosterModelError::WinsExceedMatches { .. })
        ));
    }

    #[test]
    fn derives_roster_rates_with_integer_precision() {
        let stats = HistoricalStats {
            matches: 5,
            competitive_tier: Some(18),
            account_level: Some(142),
            decided_matches: 5,
            wins: 3,
            kills: 47,
            deaths: 40,
            assists: 12,
            headshots: 30,
            bodyshots: 60,
            legshots: 10,
            kast_rounds: 35,
            rounds_played: 50,
            recent: vec![MatchOutcome::Win, MatchOutcome::Loss, MatchOutcome::Win],
        };
        assert_eq!(stats.kd_hundredths(), Some(117));
        assert_eq!(stats.win_rate_tenths(), Some(600));
        assert_eq!(stats.headshot_rate_tenths(), Some(300));
        assert_eq!(stats.kast_rate_tenths(), Some(700));
    }
}
