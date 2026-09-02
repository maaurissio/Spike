//! Normalización de la respuesta post-partida `match-details`.
//!
//! Este módulo no realiza solicitudes HTTP. Convierte un JSON ya obtenido por
//! una fuente futura a los modelos internos y rechaza respuestas incompletas.
#![allow(dead_code)] // Se conecta cuando MatchDetailSource obtenga respuestas post-partida.

use std::{collections::HashMap, time::Duration};

use reqwest::{StatusCode, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    models::{
        GameMode, MatchRounds, PlayerMatchStats, PlayerRoundStat, Round, RoundCeremony,
        RoundResult, Team,
    },
    providers::ProviderError,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9";

/// Credenciales y metadatos efímeros para una única consulta post-partida.
/// Nunca se serializan, se imprimen ni se persisten.
#[derive(Clone)]
pub(crate) struct MatchDetailRequest {
    pub match_id: String,
    pub shard: String,
    pub client_version: String,
    pub access_token: String,
    pub entitlement_token: String,
    pub own_puuid: String,
}

impl std::fmt::Debug for MatchDetailRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatchDetailRequest")
            .field("match_id", &"<redacted>")
            .field("shard", &self.shard)
            .field("client_version", &self.client_version)
            .field("access_token", &"<redacted>")
            .field("entitlement_token", &"<redacted>")
            .field("own_puuid", &"<redacted>")
            .finish()
    }
}

/// Resultado de postpartida listo para la interfaz del jugador.
/// El identificador propio nunca se muestra ni se persiste.
pub(crate) struct CompletedMatch {
    pub own_puuid: String,
    pub rounds: Option<MatchRounds>,
    pub summary: Option<MatchSummary>,
    pub totals: OwnMatchTotals,
}

/// Resumen final propio de un modo que no tiene timeline de rondas.
pub(crate) struct MatchSummary {
    pub mode: GameMode,
    pub stats: PlayerMatchStats,
}

/// Totales propios de una partida, aptos para agregados sin retener roster.
pub(crate) struct OwnMatchTotals {
    pub stats: crate::models::PlayerMatch,
    pub map: String,
    pub agent: String,
    pub own_score: Option<u32>,
    pub opponent_score: Option<u32>,
    pub roster: Vec<CompletedRosterPlayer>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum CompletedPlayerSide {
    Ally,
    Enemy,
    Participant,
}

/// Fila final del marcador, normalizada antes de abandonar el proveedor.
/// No contiene PUUID, MatchID ni PartyID.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CompletedRosterPlayer {
    pub side: CompletedPlayerSide,
    pub slot: u8,
    pub is_self: bool,
    pub name: String,
    /// Riot ID público resuelto para construir enlaces; nunca es un PUUID.
    pub riot_id: Option<String>,
    pub agent: String,
    pub rank: Option<String>,
    pub stats: PlayerMatchStats,
    pub rounds_played: u32,
    /// Índice visual estable (1..=5); `None` significa jugador solo/sin dato.
    pub premade: Option<u8>,
}

/// Consulta de solo lectura a `pd.{shard}.a.pvp.net` para una partida concluida.
pub(crate) struct MatchDetailSource {
    client: Client,
    base_url: Option<String>,
}

impl MatchDetailSource {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de postpartida debe poder construirse"),
            base_url: None,
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de prueba debe poder construirse"),
            base_url: Some(base_url),
        }
    }

    pub(crate) fn fetch_completed(
        &self,
        request: &MatchDetailRequest,
    ) -> Result<CompletedMatch, ProviderError> {
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/match-details/v1/matches/{}", request.match_id);
        let response = self
            .client
            .get(url)
            .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
            .header("X-Riot-ClientVersion", &request.client_version)
            .header("X-Riot-Entitlements-JWT", &request.entitlement_token)
            .bearer_auth(&request.access_token)
            .send()
            .map_err(|_| ProviderError::Network("no se pudo conectar a PD".into()))?;

        match response.status() {
            status if status.is_success() => response
                .json::<Value>()
                .map_err(|_| parse_error("JSON inválido en match-details"))
                .and_then(|payload| parse_completed_match(&payload, &request.own_puuid)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "PD rechazó las credenciales de sesión".into(),
            )),
            StatusCode::NOT_FOUND => Err(ProviderError::EndpointUnavailable {
                endpoint: "/match-details/v1/matches/<redacted>".into(),
                status: StatusCode::NOT_FOUND.as_u16(),
            }),
            status => Err(ProviderError::Unavailable(format!(
                "PD respondió HTTP {status} en match-details"
            ))),
        }
    }

    pub(crate) fn fetch_own_totals(
        &self,
        request: &MatchDetailRequest,
    ) -> Result<OwnMatchTotals, ProviderError> {
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/match-details/v1/matches/{}", request.match_id);
        let response = self
            .client
            .get(url)
            .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
            .header("X-Riot-ClientVersion", &request.client_version)
            .header("X-Riot-Entitlements-JWT", &request.entitlement_token)
            .bearer_auth(&request.access_token)
            .send()
            .map_err(|_| ProviderError::Network("no se pudo conectar a PD".into()))?;
        match response.status() {
            status if status.is_success() => {
                let payload = response
                    .json::<Value>()
                    .map_err(|_| parse_error("JSON inválido en match-details"))?;
                let subjects = scoreboard_subjects(&payload);
                let names = self.fetch_names(request, &subjects).unwrap_or_default();
                let mut totals = parse_own_match_totals(&payload, &request.own_puuid)?;
                for (player, subject) in totals.roster.iter_mut().zip(subjects) {
                    if let Some(riot_id) = names.get(&subject) {
                        player.riot_id = Some(riot_id.clone());
                        player.name = riot_id.clone();
                    }
                }
                Ok(totals)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "PD rechazó las credenciales de sesión".into(),
            )),
            status => Err(ProviderError::Unavailable(format!(
                "PD respondió HTTP {status} en match-details"
            ))),
        }
    }

    fn fetch_names(
        &self,
        request: &MatchDetailRequest,
        subjects: &[String],
    ) -> Result<HashMap<String, String>, ProviderError> {
        if subjects.is_empty() {
            return Ok(HashMap::new());
        }
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let response = self
            .client
            .put(format!("{base}/name-service/v2/players"))
            .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
            .header("X-Riot-ClientVersion", &request.client_version)
            .header("X-Riot-Entitlements-JWT", &request.entitlement_token)
            .bearer_auth(&request.access_token)
            .json(subjects)
            .send()
            .map_err(|_| {
                ProviderError::Network("no se pudo resolver el roster histórico".into())
            })?;
        match response.status() {
            status if status.is_success() => response
                .json::<Value>()
                .map(|payload| crate::providers::roster::visible_names(&payload))
                .map_err(|_| parse_error("JSON inválido en Name Service")),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "Name Service rechazó la sesión".into(),
            )),
            status => Err(ProviderError::Unavailable(format!(
                "Name Service respondió HTTP {status}"
            ))),
        }
    }
}

fn parse_completed_match(
    payload: &Value,
    own_puuid: &str,
) -> Result<CompletedMatch, ProviderError> {
    let mode = completed_game_mode(payload)?;
    let totals = parse_own_match_totals(payload, own_puuid)?;
    let (rounds, summary) = if mode.supports_round_timeline() {
        (Some(parse_completed_match_details(payload)?), None)
    } else {
        (None, Some(parse_match_summary(payload, mode, own_puuid)?))
    };
    Ok(CompletedMatch {
        own_puuid: own_puuid.to_owned(),
        rounds,
        summary,
        totals,
    })
}

impl Default for MatchDetailSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Convierte una respuesta finalizada de `match-details` en un timeline de rondas.
///
/// No acepta partidas activas: `match-details` se consume únicamente al terminar
/// la partida para evitar presentar estadísticas parciales como definitivas.
pub(crate) fn parse_completed_match_details(payload: &Value) -> Result<MatchRounds, ProviderError> {
    let match_info = required_object(payload, "matchInfo")?;
    let match_id = required_text(match_info, "matchId")?;
    let mode = completed_game_mode(payload)?;
    let raw_rounds = payload
        .get("roundResults")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_error("roundResults ausente o inválido"))?;

    let mut rounds = Vec::with_capacity(raw_rounds.len());
    for (index, raw_round) in raw_rounds.iter().enumerate() {
        let expected = u32::try_from(index + 1).map_err(|_| parse_error("demasiadas rondas"))?;
        rounds.push(parse_round(raw_round, index, expected)?);
    }
    MatchRounds::new(match_id.to_owned(), mode, rounds)
        .map_err(|error| parse_error(&format!("timeline inválido: {error}")))
}

fn completed_game_mode(payload: &Value) -> Result<GameMode, ProviderError> {
    let match_info = required_object(payload, "matchInfo")?;
    if !match_info
        .get("isCompleted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(parse_error("match-details aún no está finalizado"));
    }
    let queue = match_info
        .get("queueID")
        .or_else(|| match_info.get("queueId"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    if let Some(queue) = queue {
        let mode = parse_game_mode(queue)?;
        if mode != GameMode::Unknown {
            return Ok(mode);
        }
    }
    parse_game_mode(required_text(match_info, "gameMode")?)
}

fn parse_match_summary(
    payload: &Value,
    mode: GameMode,
    own_puuid: &str,
) -> Result<MatchSummary, ProviderError> {
    let player = payload
        .get("players")
        .and_then(Value::as_array)
        .and_then(|players| {
            players
                .iter()
                .find(|player| player.get("subject").and_then(Value::as_str) == Some(own_puuid))
        })
        .and_then(Value::as_object)
        .ok_or_else(|| parse_error("estadísticas propias ausentes en match-details"))?;
    let stats = player
        .get("stats")
        .and_then(Value::as_object)
        .ok_or_else(|| parse_error("stats propias ausentes en match-details"))?;
    let combat = own_combat_totals(payload, own_puuid);
    Ok(MatchSummary {
        mode,
        stats: PlayerMatchStats {
            kills: required_u32(stats, "kills")?,
            deaths: required_u32(stats, "deaths")?,
            assists: required_u32(stats, "assists")?,
            combat_score: optional_u32(stats, "score")?,
            damage: combat.damage,
            headshots: combat.headshots,
            bodyshots: combat.bodyshots,
            legshots: combat.legshots,
        },
    })
}

fn parse_own_match_totals(
    payload: &Value,
    own_puuid: &str,
) -> Result<OwnMatchTotals, ProviderError> {
    let match_info = required_object(payload, "matchInfo")?;
    completed_game_mode(payload)?;
    let map = required_text(match_info, "mapId").map(crate::providers::live_match::asset_label)?;
    let player = payload
        .get("players")
        .and_then(Value::as_array)
        .and_then(|players| {
            players
                .iter()
                .find(|player| player.get("subject").and_then(Value::as_str) == Some(own_puuid))
        })
        .and_then(Value::as_object)
        .ok_or_else(|| parse_error("estadísticas propias ausentes en match-details"))?;
    let stats = player
        .get("stats")
        .and_then(Value::as_object)
        .ok_or_else(|| parse_error("stats propias ausentes en match-details"))?;
    let agent = player
        .get("characterId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(crate::providers::live_match::agent_label)
        .unwrap_or_else(|| "no disponible".into());
    let own_team_id = player.get("teamId").and_then(Value::as_str);
    let teams = payload.get("teams").and_then(Value::as_array);
    let own_team = own_team_id.and_then(|team_id| {
        teams.and_then(|teams| {
            teams
                .iter()
                .find(|team| team.get("teamId").and_then(Value::as_str) == Some(team_id))
        })
    });
    let opponent = own_team_id.and_then(|team_id| {
        teams.and_then(|teams| {
            teams
                .iter()
                .find(|team| team.get("teamId").and_then(Value::as_str) != Some(team_id))
        })
    });
    let outcome = own_team
        .and_then(|team| team.get("won").and_then(Value::as_bool))
        .map(|won| {
            if won {
                crate::models::MatchOutcome::Win
            } else {
                crate::models::MatchOutcome::Loss
            }
        })
        .unwrap_or(crate::models::MatchOutcome::Unknown);
    let combat = own_combat_totals(payload, own_puuid);
    let roster = completed_roster(payload, own_puuid, own_team_id);
    Ok(OwnMatchTotals {
        stats: crate::models::PlayerMatch {
            outcome,
            rounds_played: optional_u32(stats, "roundsPlayed")?.unwrap_or(0),
            stats: crate::models::PlayerMatchStats {
                kills: required_u32(stats, "kills")?,
                deaths: required_u32(stats, "deaths")?,
                assists: required_u32(stats, "assists")?,
                combat_score: optional_u32(stats, "score")?,
                damage: combat.damage,
                headshots: combat.headshots,
                bodyshots: combat.bodyshots,
                legshots: combat.legshots,
            },
        },
        map,
        agent,
        own_score: own_team
            .and_then(|team| team.get("roundsWon"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        opponent_score: opponent
            .and_then(|team| team.get("roundsWon"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        roster,
    })
}

fn completed_roster(
    payload: &Value,
    own_puuid: &str,
    own_team_id: Option<&str>,
) -> Vec<CompletedRosterPlayer> {
    let Some(players) = payload.get("players").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut party_counts = HashMap::<String, usize>::new();
    for player in players {
        if let Some(party) = completed_party_id(player) {
            *party_counts.entry(party.to_owned()).or_default() += 1;
        }
    }
    let mut party_indexes = HashMap::<String, u8>::new();
    let mut next_party = 1_u8;
    let mut ally_slot = 0_u8;
    let mut enemy_slot = 0_u8;
    let mut participant_slot = 0_u8;

    players
        .iter()
        .filter_map(|player| {
            let subject = player.get("subject").and_then(Value::as_str)?;
            let stats = player.get("stats").and_then(Value::as_object)?;
            let team = player.get("teamId").and_then(Value::as_str);
            let side = match own_team_id {
                Some(own) if team == Some(own) => {
                    ally_slot = ally_slot.saturating_add(1);
                    CompletedPlayerSide::Ally
                }
                Some(_) if team.is_some() => {
                    enemy_slot = enemy_slot.saturating_add(1);
                    CompletedPlayerSide::Enemy
                }
                _ => {
                    participant_slot = participant_slot.saturating_add(1);
                    CompletedPlayerSide::Participant
                }
            };
            let slot = match side {
                CompletedPlayerSide::Ally => ally_slot,
                CompletedPlayerSide::Enemy => enemy_slot,
                CompletedPlayerSide::Participant => participant_slot,
            };
            let is_self = subject == own_puuid;
            let game_name = player
                .get("gameName")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty());
            let tag = player
                .get("tagLine")
                .and_then(Value::as_str)
                .filter(|tag| !tag.is_empty());
            let name = if let Some(game_name) = game_name {
                tag.map_or_else(|| game_name.into(), |tag| format!("{game_name}#{tag}"))
            } else if is_self {
                "Tú".into()
            } else {
                format!("Jugador {slot}")
            };
            let combat = own_combat_totals(payload, subject);
            let rounds_played = optional_u32(stats, "roundsPlayed")
                .ok()
                .flatten()
                .unwrap_or(0);
            let premade = completed_party_id(player)
                .filter(|party| party_counts.get(*party).copied().unwrap_or(0) > 1)
                .map(|party| {
                    *party_indexes.entry(party.to_owned()).or_insert_with(|| {
                        let index = next_party;
                        next_party = next_party.saturating_add(1);
                        index
                    })
                });
            Some(CompletedRosterPlayer {
                side,
                slot,
                is_self,
                name,
                riot_id: game_name.map(|game_name| {
                    tag.map_or_else(|| game_name.into(), |tag| format!("{game_name}#{tag}"))
                }),
                agent: player
                    .get("characterId")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(crate::providers::live_match::agent_label)
                    .unwrap_or_else(|| "—".into()),
                rank: player
                    .get("competitiveTier")
                    .or_else(|| player.get("CompetitiveTier"))
                    .and_then(Value::as_u64)
                    .and_then(crate::providers::roster::competitive_tier_label),
                stats: PlayerMatchStats {
                    kills: required_u32(stats, "kills").ok()?,
                    deaths: required_u32(stats, "deaths").ok()?,
                    assists: required_u32(stats, "assists").ok()?,
                    combat_score: optional_u32(stats, "score").ok().flatten(),
                    damage: combat.damage,
                    headshots: combat.headshots,
                    bodyshots: combat.bodyshots,
                    legshots: combat.legshots,
                },
                rounds_played,
                premade,
            })
        })
        .collect()
}

fn scoreboard_subjects(payload: &Value) -> Vec<String> {
    payload
        .get("players")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|player| player.get("stats").and_then(Value::as_object).is_some())
        .filter_map(|player| player.get("subject").and_then(Value::as_str))
        .filter(|subject| !subject.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn completed_party_id(player: &Value) -> Option<&str> {
    ["partyId", "partyID", "PartyID"]
        .iter()
        .find_map(|field| player.get(*field).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
}

#[derive(Default)]
struct OwnCombatTotals {
    damage: Option<u32>,
    headshots: Option<u32>,
    bodyshots: Option<u32>,
    legshots: Option<u32>,
}

/// Suma únicamente el daño saliente del jugador autenticado. Las respuestas
/// Ranked incluyen estos impactos por ronda; si el proveedor omite los campos,
/// se conserva `None` para no mostrar un HS% parcial o inventado.
fn own_combat_totals(payload: &Value, own_puuid: &str) -> OwnCombatTotals {
    let mut damage = 0_u32;
    let mut headshots = 0_u32;
    let mut bodyshots = 0_u32;
    let mut legshots = 0_u32;
    let mut saw_damage = false;
    let mut saw_shots = false;
    let mut shots_complete = true;

    let Some(rounds) = payload.get("roundResults").and_then(Value::as_array) else {
        return OwnCombatTotals::default();
    };
    for round in rounds {
        let own = round
            .get("playerStats")
            .and_then(Value::as_array)
            .and_then(|players| {
                players
                    .iter()
                    .find(|player| player.get("subject").and_then(Value::as_str) == Some(own_puuid))
            });
        let Some(events) = own
            .and_then(|player| player.get("damage"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for event in events {
            if let Some(value) = event
                .get("damage")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
            {
                damage = damage.saturating_add(value);
                saw_damage = true;
            }
            let shots = ["headshots", "bodyshots", "legshots"].map(|field| {
                event
                    .get(field)
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
            });
            if shots.iter().any(Option::is_some) {
                if let [Some(head), Some(body), Some(leg)] = shots {
                    headshots = headshots.saturating_add(head);
                    bodyshots = bodyshots.saturating_add(body);
                    legshots = legshots.saturating_add(leg);
                    saw_shots = true;
                } else {
                    shots_complete = false;
                }
            }
        }
    }

    OwnCombatTotals {
        damage: saw_damage.then_some(damage),
        headshots: (saw_shots && shots_complete).then_some(headshots),
        bodyshots: (saw_shots && shots_complete).then_some(bodyshots),
        legshots: (saw_shots && shots_complete).then_some(legshots),
    }
}

fn parse_round(raw: &Value, index: usize, expected: u32) -> Result<Round, ProviderError> {
    let object = raw
        .as_object()
        .ok_or_else(|| parse_error("ronda inválida"))?;
    let raw_number = required_u32(object, "roundNum")?;
    let zero_based = u32::try_from(index).map_err(|_| parse_error("índice de ronda inválido"))?;
    if raw_number != zero_based && raw_number != expected {
        return Err(parse_error(
            "roundNum no coincide con el orden de la respuesta",
        ));
    }
    let player_stats = object
        .get("playerStats")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_error("playerStats ausente o inválido"))?;
    let deaths = death_counts(player_stats);
    let players = player_stats
        .iter()
        .map(|player| parse_player_round_stat(player, &deaths))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Round {
        round_num: expected,
        winning_team: parse_team(required_text(object, "winningTeam")?)?,
        round_result: parse_round_result(required_text(object, "roundResult")?),
        ceremony: object
            .get("roundCeremony")
            .and_then(Value::as_str)
            .and_then(parse_ceremony),
        players,
    })
}

fn death_counts(players: &[Value]) -> HashMap<String, u8> {
    let mut deaths = HashMap::new();
    for player in players {
        let Some(kills) = player.get("kills").and_then(Value::as_array) else {
            continue;
        };
        for kill in kills {
            let Some(victim) = kill.get("victim").and_then(Value::as_str) else {
                continue;
            };
            deaths
                .entry(victim.to_owned())
                .and_modify(|count: &mut u8| *count = count.saturating_add(1))
                .or_insert(1);
        }
    }
    deaths
}

fn parse_player_round_stat(
    raw: &Value,
    deaths: &HashMap<String, u8>,
) -> Result<PlayerRoundStat, ProviderError> {
    let object = raw
        .as_object()
        .ok_or_else(|| parse_error("estadística de jugador inválida"))?;
    let subject = required_text(object, "subject")?;
    let kills = object
        .get("kills")
        .and_then(Value::as_array)
        .map(|entries| entries.len())
        .ok_or_else(|| parse_error("kills ausente o inválido"))?;
    let damage = object
        .get("damage")
        .and_then(Value::as_array)
        .map(|entries| {
            entries.iter().fold(0_u32, |total, entry| {
                total
                    .saturating_add(entry.get("damage").and_then(Value::as_u64).unwrap_or(0) as u32)
            })
        });
    Ok(PlayerRoundStat {
        puuid: subject.to_owned(),
        kills: u8::try_from(kills).map_err(|_| parse_error("demasiados kills en una ronda"))?,
        deaths: deaths.get(subject).copied().unwrap_or(0),
        score: optional_u32(object, "score")?,
        damage,
    })
}

fn parse_game_mode(value: &str) -> Result<GameMode, ProviderError> {
    let value = value.to_ascii_lowercase();
    let mode = match value.as_str() {
        "competitive" => Ok(GameMode::Competitive),
        "unrated" | "standard" => Ok(GameMode::Unrated),
        "customgame" | "custom" => Ok(GameMode::Custom),
        "swiftplay" => Ok(GameMode::Swiftplay),
        "deathmatch" => Ok(GameMode::Deathmatch),
        "teamdeathmatch" => Ok(GameMode::TeamDeathmatch),
        "escalation" => Ok(GameMode::Escalation),
        _ if value.contains("bombgamemode") || value.ends_with("/bomb") => Ok(GameMode::Unrated),
        _ if value.contains("teamdeathmatch") || value.contains("hurm") => {
            Ok(GameMode::TeamDeathmatch)
        }
        _ if value.contains("deathmatch") => Ok(GameMode::Deathmatch),
        _ if value.contains("swiftplay") => Ok(GameMode::Swiftplay),
        _ if value.contains("escalation") || value.contains("gunprogression") => {
            Ok(GameMode::Escalation)
        }
        _ => Ok(GameMode::Unknown),
    }?;
    Ok(mode)
}

fn parse_team(value: &str) -> Result<Team, ProviderError> {
    match value {
        "Blue" => Ok(Team::Blue),
        "Red" => Ok(Team::Red),
        _ => Err(parse_error("equipo ganador desconocido")),
    }
}

fn parse_round_result(value: &str) -> RoundResult {
    match value {
        "Eliminated" => RoundResult::Eliminated,
        "Bomb detonated" => RoundResult::Detonated,
        "Bomb defused" => RoundResult::Defused,
        "Surrendered" => RoundResult::Surrendered,
        "Round timer expired" => RoundResult::TimerExpired,
        _ => RoundResult::Unknown,
    }
}

fn parse_ceremony(value: &str) -> Option<RoundCeremony> {
    match value {
        "CeremonyAce" => Some(RoundCeremony::Ace),
        "CeremonyTeamAce" => Some(RoundCeremony::TeamAce),
        "CeremonyClutch" => Some(RoundCeremony::Clutch),
        "CeremonyFlawless" => Some(RoundCeremony::Flawless),
        "CeremonyThrifty" => Some(RoundCeremony::Thrifty),
        "CeremonyCloser" => Some(RoundCeremony::Closer),
        _ => None,
    }
}

fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, ProviderError> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| parse_error(&format!("{field} ausente o inválido")))
}

fn required_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, ProviderError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| parse_error(&format!("{field} ausente o vacío")))
}

fn required_u32(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u32, ProviderError> {
    optional_u32(object, field)?.ok_or_else(|| parse_error(&format!("{field} ausente o inválido")))
}

fn optional_u32(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<u32>, ProviderError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| parse_error(&format!("{field} fuera de rango o inválido"))),
    }
}

fn parse_error(message: &str) -> ProviderError {
    ProviderError::Parse(message.into())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    fn fixture() -> Value {
        serde_json::json!({
            "matchInfo": {"matchId": "match", "mapId": "/Game/Maps/Ascent/Ascent", "gameMode": "Competitive", "isCompleted": true},
            "players": [
                {"subject":"me", "teamId":"Blue", "characterId":"8e253930-4c05-31dd-1b6c-968525494517", "stats":{"kills":1,"deaths":1,"assists":0,"roundsPlayed":2,"score":400}},
                {"subject":"them", "teamId":"Red", "stats":{"kills":1,"deaths":1,"assists":0,"roundsPlayed":2,"score":250}}
            ],
            "teams": [
                {"teamId":"Blue", "won":true, "roundsWon":1},
                {"teamId":"Red", "won":false, "roundsWon":1}
            ],
            "roundResults": [
                {
                    "roundNum": 0,
                    "roundResult": "Eliminated",
                    "roundCeremony": "CeremonyAce",
                    "winningTeam": "Blue",
                    "playerStats": [
                        {"subject": "me", "kills": [{"killer": "me", "victim": "them"}], "damage": [{"receiver": "them", "damage": 150, "headshots": 1, "bodyshots": 1, "legshots": 0}], "score": 300},
                        {"subject": "them", "kills": [], "damage": [], "score": 0}
                    ]
                },
                {
                    "roundNum": 2,
                    "roundResult": "Bomb defused",
                    "roundCeremony": "",
                    "winningTeam": "Red",
                    "playerStats": [
                        {"subject": "me", "kills": [], "damage": [], "score": 100},
                        {"subject": "them", "kills": [{"killer": "them", "victim": "me"}], "damage": [{"receiver": "me", "damage": 100}], "score": 250}
                    ]
                }
            ]
        })
    }

    #[test]
    fn normalizes_completed_rounds_and_accepts_zero_or_one_based_numbers() {
        let result = parse_completed_match_details(&fixture()).unwrap();

        assert_eq!(result.match_id, "match");
        assert_eq!(result.mode, GameMode::Competitive);
        assert_eq!(result.rounds[0].round_num, 1);
        assert_eq!(result.rounds[0].round_result, RoundResult::Eliminated);
        assert_eq!(result.rounds[0].ceremony, Some(RoundCeremony::Ace));
        assert_eq!(result.rounds[0].players[0].kills, 1);
        assert_eq!(result.rounds[0].players[0].deaths, 0);
        assert_eq!(result.rounds[0].players[0].damage, Some(150));
        assert_eq!(result.rounds[1].round_num, 2);
        assert_eq!(result.rounds[1].players[0].deaths, 1);
    }

    #[test]
    fn derives_own_damage_and_headshot_fields_from_ranked_rounds() {
        let totals = parse_own_match_totals(&fixture(), "me").unwrap();

        assert_eq!(totals.stats.stats.damage, Some(150));
        assert_eq!(totals.stats.stats.headshots, Some(1));
        assert_eq!(totals.stats.stats.bodyshots, Some(1));
        assert_eq!(totals.stats.stats.legshots, Some(0));
    }

    #[test]
    fn rejects_match_that_is_not_complete() {
        let mut payload = fixture();
        payload["matchInfo"]["isCompleted"] = Value::Bool(false);

        let error = parse_completed_match_details(&payload).unwrap_err();

        assert!(error.to_string().contains("aún no está finalizado"));
    }

    #[test]
    fn rejects_round_number_outside_known_conventions() {
        let mut payload = fixture();
        payload["roundResults"][1]["roundNum"] = Value::from(7);

        let error = parse_completed_match_details(&payload).unwrap_err();

        assert!(error.to_string().contains("roundNum no coincide"));
    }

    #[test]
    fn summarizes_own_deathmatch_without_retaining_a_round_timeline() {
        let payload = serde_json::json!({
            "matchInfo": {"matchId": "dm", "mapId": "/Game/Maps/Ascent/Ascent", "gameMode": "Deathmatch", "isCompleted": true},
            "players": [
                {"subject": "other", "stats": {"kills": 40, "deaths": 10, "assists": 2, "score": 4000}},
                {"subject": "me", "characterId":"8e253930-4c05-31dd-1b6c-968525494517", "stats": {"kills": 25, "deaths": 20, "assists": 4, "score": 2500}}
            ]
        });

        let completed = parse_completed_match(&payload, "me").unwrap();
        let summary = completed.summary.unwrap();

        assert!(completed.rounds.is_none());
        assert_eq!(summary.mode, GameMode::Deathmatch);
        assert_eq!(summary.stats.kills, 25);
        assert_eq!(summary.stats.deaths, 20);
        assert_eq!(summary.stats.assists, 4);
        assert_eq!(summary.stats.combat_score, Some(2500));
    }

    #[test]
    fn parses_safe_scoreboard_without_retaining_match_or_party_ids() {
        let totals = parse_own_match_totals(
            &serde_json::json!({
                "matchInfo": {"matchId": "private", "mapId": "/Game/Maps/Ascent/Ascent", "gameMode": "Competitive", "isCompleted": true},
                "players": [
                    {"subject": "other", "gameName":"Rival", "tagLine":"LAS", "partyId":"enemy-party", "teamId": "Red", "competitiveTier":21, "stats": {"kills": 30, "deaths": 10, "assists": 2, "roundsPlayed": 20, "score": 5000}},
                    {"subject": "other-two", "partyId":"enemy-party", "teamId": "Red", "stats": {"kills": 10, "deaths": 20, "assists": 4, "roundsPlayed": 20, "score": 2000}},
                    {"subject": "me", "partyId":"own-party", "teamId": "Blue", "characterId": "8e253930-4c05-31dd-1b6c-968525494517", "stats": {"kills": 20, "deaths": 15, "assists": 5, "roundsPlayed": 20, "score": 4000}},
                    {"subject": "ally", "partyId":"own-party", "teamId": "Blue", "stats": {"kills": 15, "deaths": 15, "assists": 8, "roundsPlayed": 20, "score": 3000}}
                ],
                "teams": [{"teamId": "Blue", "won": true, "roundsWon": 13}, {"teamId": "Red", "won": false, "roundsWon": 9}]
            }),
            "me",
        )
        .unwrap();

        assert_eq!(totals.stats.outcome, crate::models::MatchOutcome::Win);
        assert_eq!(totals.stats.stats.kills, 20);
        assert_eq!(totals.stats.stats.assists, 5);
        assert_eq!(totals.map, "Ascent");
        assert_eq!(totals.agent, "Omen");
        assert_eq!(totals.roster.len(), 4);
        assert_eq!(totals.roster[0].name, "Rival#LAS");
        assert_eq!(totals.roster[0].riot_id.as_deref(), Some("Rival#LAS"));
        assert_eq!(totals.roster[0].rank.as_deref(), Some("Ascendente 1"));
        assert_eq!(totals.roster[2].name, "Tú");
        assert_eq!(totals.roster[0].premade, totals.roster[1].premade);
        assert_eq!(totals.roster[2].premade, totals.roster[3].premade);
        assert_ne!(totals.roster[0].premade, totals.roster[2].premade);
        assert_eq!(
            totals
                .roster
                .iter()
                .map(|player| player.stats.combat_score)
                .collect::<Vec<_>>(),
            [Some(5000), Some(2000), Some(4000), Some(3000)]
        );
        let debug = format!("{:?}", totals.roster);
        assert!(!debug.contains("enemy-party") && !debug.contains("own-party"));
        assert_eq!(
            (totals.own_score, totals.opponent_score),
            (Some(13), Some(9))
        );
    }

    #[test]
    fn prefers_queue_over_internal_bomb_mode_name() {
        let payload = serde_json::json!({
            "matchInfo": {
                "gameMode":"/Game/GameModes/Bomb/BombGameMode.BombGameMode_C",
                "queueID":"competitive",
                "isCompleted":true
            }
        });

        assert_eq!(
            completed_game_mode(&payload).unwrap(),
            GameMode::Competitive
        );
    }

    #[test]
    fn postmatch_source_sends_required_headers_and_normalizes_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
            assert!(request.starts_with("get /match-details/v1/matches/match-id http/1.1"));
            assert!(request.contains("authorization: bearer access"));
            assert!(request.contains("x-riot-entitlements-jwt: entitlement"));
            assert!(request.contains("x-riot-clientversion: version"));
            let body = fixture().to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let source = MatchDetailSource::with_base_url(format!("http://{address}"));
        let request = MatchDetailRequest {
            match_id: "match-id".into(),
            shard: "na".into(),
            client_version: "version".into(),
            access_token: "access".into(),
            entitlement_token: "entitlement".into(),
            own_puuid: "me".into(),
        };

        let completed = source.fetch_completed(&request).unwrap();

        assert_eq!(completed.rounds.unwrap().rounds.len(), 2);
        assert_eq!(completed.own_puuid, "me");
        server.join().unwrap();
    }
}
