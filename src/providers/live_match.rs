//! Contexto mínimo y de solo lectura de la partida en curso.

use std::{collections::HashMap, time::Duration};

use reqwest::{StatusCode, blocking::Client};
use serde_json::Value;

use super::{
    ProviderError,
    roster::{competitive_tier_label, visible_names},
    roster_stats::{RosterStatsRequest, RosterStatsSource},
};
use crate::models::roster::{
    DataAvailability, HistoricalStats, RosterPlayer, RosterSide, RosterSnapshot,
};

const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9";

pub(crate) struct LiveMatchRequest {
    pub match_id: String,
    pub region: String,
    pub shard: String,
    pub client_version: String,
    pub access_token: String,
    pub entitlement_token: String,
    pub own_puuid: String,
}

pub(crate) struct LiveMatchSource {
    client: Client,
    stats: RosterStatsSource,
}

impl LiveMatchSource {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("cliente live"),
            stats: RosterStatsSource::new(),
        }
    }

    pub(crate) fn fetch(
        &self,
        request: &LiveMatchRequest,
    ) -> Result<LiveMatchContext, ProviderError> {
        let url = format!(
            "https://glz-{}-1.{}.a.pvp.net/core-game/v1/matches/{}",
            request.region, request.shard, request.match_id
        );
        let response = self
            .client
            .get(url)
            .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
            .header("X-Riot-ClientVersion", &request.client_version)
            .header("X-Riot-Entitlements-JWT", &request.entitlement_token)
            .bearer_auth(&request.access_token)
            .send()
            .map_err(|_| ProviderError::Network("no se pudo conectar a GLZ".into()))?;
        match response.status() {
            status if status.is_success() => {
                let payload = response
                    .json::<Value>()
                    .map_err(|_| ProviderError::Parse("JSON inválido en partida actual".into()))?;
                let names = self
                    .fetch_visible_names(request, &payload)
                    .unwrap_or_default();
                let subjects = roster_subjects(&payload);
                let stats_request = RosterStatsRequest {
                    shard: request.shard.clone(),
                    client_version: request.client_version.clone(),
                    access_token: request.access_token.clone(),
                    entitlement_token: request.entitlement_token.clone(),
                };
                let queue = history_queue(&payload);
                let stats = self
                    .stats
                    .fetch(&stats_request, &subjects, queue.as_deref());
                parse_live_match_with_names_and_stats(&payload, &request.own_puuid, &names, &stats)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(ProviderError::Unauthorized("GLZ rechazó la sesión".into()))
            }
            status => Err(ProviderError::Unavailable(format!(
                "GLZ respondió HTTP {status}"
            ))),
        }
    }

    /// Una sola resolución por partida. Las identidades marcadas como ocultas
    /// se excluyen de la solicitud y nunca se reconstruyen.
    fn fetch_visible_names(
        &self,
        request: &LiveMatchRequest,
        match_payload: &Value,
    ) -> Result<HashMap<String, String>, ProviderError> {
        let subjects = match_payload
            .get("Players")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|player| {
                !player
                    .pointer("/PlayerIdentity/Incognito")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .filter_map(|player| player.get("Subject").and_then(Value::as_str))
            .filter(|subject| !subject.is_empty())
            .collect::<Vec<_>>();
        if subjects.is_empty() {
            return Ok(HashMap::new());
        }
        let url = format!(
            "https://pd.{}.a.pvp.net/name-service/v2/players",
            request.shard
        );
        let response = self
            .client
            .post(url)
            .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
            .header("X-Riot-ClientVersion", &request.client_version)
            .header("X-Riot-Entitlements-JWT", &request.entitlement_token)
            .bearer_auth(&request.access_token)
            .json(&subjects)
            .send()
            .map_err(|_| ProviderError::Network("no se pudo resolver el roster".into()))?;
        match response.status() {
            status if status.is_success() => response
                .json::<Value>()
                .map(|payload| visible_names(&payload))
                .map_err(|_| ProviderError::Parse("JSON inválido en Name Service".into())),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "Name Service rechazó la sesión".into(),
            )),
            status => Err(ProviderError::Unavailable(format!(
                "Name Service respondió HTTP {status}"
            ))),
        }
    }
}

/// Contexto de pantalla ya normalizado. No contiene PUUID ni MatchID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LiveMatchContext {
    pub mode: String,
    pub map: String,
    pub agent: Option<String>,
    pub roster: Option<RosterSnapshot>,
}

#[cfg(test)]
pub(crate) fn parse_live_match(
    payload: &Value,
    own_puuid: &str,
) -> Result<LiveMatchContext, ProviderError> {
    parse_live_match_with_names_and_stats(payload, own_puuid, &HashMap::new(), &HashMap::new())
}

fn parse_live_match_with_names_and_stats(
    payload: &Value,
    own_puuid: &str,
    names: &HashMap<String, String>,
    stats: &HashMap<String, HistoricalStats>,
) -> Result<LiveMatchContext, ProviderError> {
    let object = payload
        .as_object()
        .ok_or_else(|| ProviderError::Parse("partida actual inválida".into()))?;
    let mode = required_asset(object, "ModeID")?;
    let map = required_asset(object, "MapID")?;
    let players = object.get("Players").and_then(Value::as_array);
    let agent = players
        .and_then(|players| {
            players
                .iter()
                .find(|player| player.get("Subject").and_then(Value::as_str) == Some(own_puuid))
        })
        .and_then(|player| player.get("CharacterID"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(agent_label);
    let roster = players
        .filter(|players| !players.is_empty())
        .and_then(|players| normalize_roster(players, own_puuid, names, stats).ok());
    Ok(LiveMatchContext {
        mode,
        map,
        agent,
        roster,
    })
}

fn normalize_roster(
    players: &[Value],
    own_puuid: &str,
    names: &HashMap<String, String>,
    stats: &HashMap<String, HistoricalStats>,
) -> Result<RosterSnapshot, ProviderError> {
    let own_team = players
        .iter()
        .find(|player| player.get("Subject").and_then(Value::as_str) == Some(own_puuid))
        .and_then(|player| player.get("TeamID"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let distinct_teams = players
        .iter()
        .filter_map(|player| player.get("TeamID").and_then(Value::as_str))
        .filter(|team| !team.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let free_for_all = own_team.is_empty() || distinct_teams.len() < 2;
    let mut ally_slot = 0_u8;
    let mut enemy_slot = 0_u8;
    let mut participant_slot = 0_u8;

    let normalized = players
        .iter()
        .map(|player| {
            let subject = player
                .get("Subject")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let is_self = subject == own_puuid;
            let team = player
                .get("TeamID")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let (side, slot) = if free_for_all {
                participant_slot = participant_slot.saturating_add(1);
                (RosterSide::Participant, participant_slot)
            } else if team == own_team {
                ally_slot = ally_slot.saturating_add(1);
                (RosterSide::Ally, ally_slot)
            } else {
                enemy_slot = enemy_slot.saturating_add(1);
                (RosterSide::Enemy, enemy_slot)
            };
            let hidden = player
                .pointer("/PlayerIdentity/Incognito")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let identity = if is_self {
                DataAvailability::Available("Tú".into())
            } else if hidden {
                DataAvailability::Hidden
            } else {
                names
                    .get(subject)
                    .cloned()
                    .map(DataAvailability::Available)
                    .unwrap_or(DataAvailability::NotAvailable)
            };
            let agent = player
                .get("CharacterID")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(agent_label)
                .filter(|value| value != "no disponible")
                .map(DataAvailability::Available)
                .unwrap_or(DataAvailability::NotAvailable);
            let rank = player
                .get("CompetitiveTier")
                .or_else(|| player.pointer("/PlayerIdentity/CompetitiveTier"))
                .or_else(|| player.pointer("/SeasonalBadgeInfo/Rank"))
                .and_then(Value::as_u64)
                .and_then(competitive_tier_label)
                .map(DataAvailability::Available)
                .unwrap_or(DataAvailability::NotAvailable);
            RosterPlayer {
                side,
                slot,
                is_self,
                identity,
                agent,
                rank,
                stats: stats
                    .get(subject)
                    .cloned()
                    .map(DataAvailability::Available)
                    .unwrap_or(DataAvailability::NotAvailable),
            }
        })
        .collect();
    RosterSnapshot::new(normalized)
        .map_err(|error| ProviderError::Parse(format!("roster inválido: {error}")))
}

fn roster_subjects(payload: &Value) -> Vec<String> {
    payload
        .get("Players")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|player| {
            !player
                .get("IsCoach")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|player| player.get("Subject").and_then(Value::as_str))
        .filter(|subject| !subject.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// El payload live no siempre distingue normal de competitivo. Cuando la cola
/// no viene informada, las partidas con bomba muestran el histórico competitivo
/// (la referencia útil junto al rango); los modos explícitos conservan su cola.
fn history_queue(payload: &Value) -> Option<String> {
    if let Some(queue) = payload
        .pointer("/MatchmakingData/QueueID")
        .or_else(|| payload.pointer("/MatchmakingData/QueueId"))
        .and_then(Value::as_str)
        .filter(|queue| !queue.is_empty())
    {
        return Some(queue.to_owned());
    }
    if payload.get("ProvisioningFlow").and_then(Value::as_str) == Some("CustomGame") {
        return None;
    }
    let mode = payload
        .get("ModeID")
        .and_then(Value::as_str)
        .map(asset_label)?;
    match mode.to_ascii_lowercase().as_str() {
        "bomb" => Some("competitive".into()),
        "deathmatch" => Some("deathmatch".into()),
        "teamdeathmatch" => Some("teamdeathmatch".into()),
        "swiftplay" => Some("swiftplay".into()),
        "spikerush" | "quickbomb" => Some("spikerush".into()),
        "escalation" => Some("escalation".into()),
        _ => None,
    }
}

fn required_asset(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, ProviderError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(asset_label)
        .ok_or_else(|| ProviderError::Parse(format!("{field} ausente en partida actual")))
}

/// Las rutas internas de Riot suelen terminar en un nombre legible. Los UUIDs
/// de los agentes se resuelven con el catálogo integrado; no implica otra
/// consulta de red durante una partida.
pub(crate) fn asset_label(value: &str) -> String {
    let parts = value.trim_matches('/').split('/').collect::<Vec<_>>();
    let candidate = parts.last().copied().unwrap_or(value);
    if candidate
        .chars()
        .all(|character| character.is_ascii_hexdigit() || character == '-')
    {
        "no disponible".into()
    } else {
        map_name(candidate).unwrap_or(candidate).to_owned()
    }
}

/// Algunas respuestas exponen el nombre de proyecto del mapa en lugar de su
/// etiqueta pública. Este catálogo local evita solicitudes adicionales y deja
/// intacto cualquier valor nuevo que todavía no conozcamos.
fn map_name(id: &str) -> Option<&'static str> {
    match id.to_ascii_lowercase().as_str() {
        "ascent" => Some("Ascent"),
        "bonsai" => Some("Split"),
        "canyon" => Some("Fracture"),
        "duality" => Some("Bind"),
        "foxtrot" => Some("Breeze"),
        "infinity" => Some("Abyss"),
        "jam" => Some("Lotus"),
        "juliett" => Some("Sunset"),
        "plummet" => Some("Summit"),
        "pitt" => Some("Pearl"),
        "port" => Some("Icebox"),
        "rook" => Some("Corrode"),
        "triad" => Some("Haven"),
        _ => None,
    }
}

pub(crate) fn agent_label(value: &str) -> String {
    let label = asset_label(value);
    if label != "no disponible" {
        return label;
    }
    agent_name(value).unwrap_or(label)
}

/// Catálogo de agentes jugables a la fecha de esta versión. Un ID desconocido
/// se mantiene como no disponible en vez de atribuirle un nombre incorrecto.
fn agent_name(id: &str) -> Option<String> {
    let name = match id.to_ascii_lowercase().as_str() {
        "41fb69c1-4189-7b37-f117-bcaf1e96f1bf" => "Astra",
        "5f8d3a7f-467b-97f3-062c-13acf203c006" => "Breach",
        "9f0d8ba9-4140-b941-57d3-a7ad57c6b417" => "Brimstone",
        "22697a3d-45bf-8dd7-4fec-84a9e28c69d7" => "Chamber",
        "1dbf2edd-4729-0984-3115-daa5eed44993" => "Clove",
        "117ed9e3-49f3-6512-3ccf-0cada7e3823b" => "Cypher",
        "cc8b64c8-4b25-4ff9-6e7f-37b4da43d235" => "Deadlock",
        "dade69b4-4f5a-8528-247b-219e5a1facd6" => "Fade",
        "e370fa57-4757-3604-3648-499e1f642d3f" => "Gekko",
        "95b78ed7-4637-86d9-7e41-71ba8c293152" => "Harbor",
        "0e38b510-41a8-5780-5e8f-568b2a4f2d6c" => "Iso",
        "add6443a-41bd-e414-f6ad-e58d267f4e95" => "Jett",
        "601dbbe7-43ce-be57-2a40-4abd24953621" => "KAY/O",
        "1e58de9c-4950-5125-93e9-a0aee9f98746" => "Killjoy",
        "7c8a4701-4de6-9355-b254-e09bc2a34b72" => "Miks",
        "bb2a4828-46eb-8cd1-e765-15848195d751" => "Neon",
        "8e253930-4c05-31dd-1b6c-968525494517" => "Omen",
        "eb93336a-449b-9c1b-0a54-a891f7921d69" => "Phoenix",
        "f94c3b30-42be-e959-889c-5aa313dba261" => "Raze",
        "a3bfb853-43b2-7238-a4f1-ad90e9e46bcc" => "Reyna",
        "569fdd95-4d10-43ab-ca70-79becc718b46" => "Sage",
        "6f2a04ca-43e0-be17-7f36-b3908627744d" => "Skye",
        "320b2a48-4d9b-a075-30f1-1f93a9b638fa" => "Sova",
        "b444168c-4e35-8076-db47-ef9bf368f384" => "Tejo",
        "92eeef5d-43b5-1d4a-8d03-b3927a09034b" => "Veto",
        "707eab51-4836-f488-046a-cda6bf494859" => "Viper",
        "efba5359-4016-a1e5-7626-b1ae76895940" => "Vyse",
        "df1cb487-4902-002e-5c17-d28e83e78588" => "Waylay",
        "7f94d92c-4234-0a36-9646-3a87eb8b5c89" => "Yoru",
        _ => return None,
    };
    Some(name.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_own_context() {
        let context = parse_live_match(
            &serde_json::json!({
                "ModeID": "/Game/GameModes/Deathmatch/Deathmatch",
                "MapID": "/Game/Maps/Ascent/Ascent",
                "Players": [
                    {"Subject": "other", "CharacterID": "/Game/Agents/Jett"},
                    {"Subject": "me", "CharacterID": "/Game/Agents/Omen"}
                ]
            }),
            "me",
        )
        .unwrap();

        assert_eq!(context.mode, "Deathmatch");
        assert_eq!(context.map, "Ascent");
        assert_eq!(context.agent.as_deref(), Some("Omen"));
    }

    #[test]
    fn resolves_own_agent_uuid_without_exposing_other_players() {
        let context = parse_live_match(
            &serde_json::json!({
                "ModeID": "/Game/GameModes/Bomb/Bomb",
                "MapID": "/Game/Maps/Ascent/Ascent",
                "Players": [
                    {"Subject": "other", "CharacterID": "add6443a-41bd-e414-f6ad-e58d267f4e95"},
                    {"Subject": "me", "CharacterID": "8e253930-4c05-31dd-1b6c-968525494517"}
                ]
            }),
            "me",
        )
        .unwrap();

        assert_eq!(context.agent.as_deref(), Some("Omen"));
    }

    #[test]
    fn normalizes_visible_hidden_and_ranked_roster_without_ids() {
        let names = HashMap::from([
            ("me".to_owned(), "MiCuenta#LAS".to_owned()),
            ("ally".to_owned(), "Aliado#LAN".to_owned()),
            ("enemy".to_owned(), "Rival#BR".to_owned()),
            ("hidden-secret".to_owned(), "No debe aparecer".to_owned()),
        ]);
        let stats = HashMap::from([(
            "hidden-secret".to_owned(),
            HistoricalStats {
                matches: 2,
                decided_matches: 2,
                wins: 1,
                kills: 30,
                deaths: 20,
                ..Default::default()
            },
        )]);
        let context = parse_live_match_with_names_and_stats(
            &serde_json::json!({
                "ModeID": "/Game/GameModes/Bomb/Bomb",
                "MapID": "/Game/Maps/Triad/Triad",
                "Players": [
                    {"Subject":"me", "TeamID":"Blue", "CharacterID":"8e253930-4c05-31dd-1b6c-968525494517", "CompetitiveTier":18},
                    {"Subject":"ally", "TeamID":"Blue", "CharacterID":"320b2a48-4d9b-a075-30f1-1f93a9b638fa", "CompetitiveTier":12},
                    {"Subject":"enemy", "TeamID":"Red", "CharacterID":"add6443a-41bd-e414-f6ad-e58d267f4e95", "CompetitiveTier":27},
                    {"Subject":"hidden-secret", "TeamID":"Red", "CharacterID":"569fdd95-4d10-43ab-ca70-79becc718b46", "PlayerIdentity":{"Incognito":true}, "SeasonalBadgeInfo":{"Rank":6}}
                ]
            }),
            "me",
            &names,
            &stats,
        )
        .unwrap();
        let roster = context.roster.unwrap();

        assert_eq!(roster.allies().count(), 2);
        assert_eq!(roster.enemies().count(), 2);
        assert_eq!(
            roster.players[1].identity,
            DataAvailability::Available("Aliado#LAN".into())
        );
        assert_eq!(
            roster.players[2].rank,
            DataAvailability::Available("Radiante".into())
        );
        assert_eq!(roster.players[3].identity, DataAvailability::Hidden);
        assert!(matches!(
            &roster.players[3].stats,
            DataAvailability::Available(stats) if stats.kd_hundredths() == Some(150)
        ));
        let debug = format!("{roster:?}");
        assert!(!debug.contains("hidden-secret") && !debug.contains("No debe aparecer"));
    }

    #[test]
    fn normalizes_deathmatch_as_participants_instead_of_fake_teams() {
        let context = parse_live_match(
            &serde_json::json!({
                "ModeID": "/Game/GameModes/Deathmatch/Deathmatch",
                "MapID": "/Game/Maps/Ascent/Ascent",
                "Players": [
                    {"Subject":"me", "TeamID":"Neutral", "CharacterID":"/Game/Agents/Omen"},
                    {"Subject":"other", "TeamID":"Neutral", "CharacterID":"/Game/Agents/Jett"}
                ]
            }),
            "me",
        )
        .unwrap();
        let roster = context.roster.unwrap();
        assert_eq!(roster.participants().count(), 2);
        assert_eq!(roster.allies().count(), 0);
        assert_eq!(roster.enemies().count(), 0);
    }

    #[test]
    fn resolves_internal_map_names_to_public_labels() {
        assert_eq!(asset_label("/Game/Maps/Juliett/Juliett"), "Sunset");
        assert_eq!(asset_label("/Game/Maps/Triad/Triad"), "Haven");
        assert_eq!(asset_label("/Game/Maps/Plummet/Plummet"), "Summit");
    }
}
