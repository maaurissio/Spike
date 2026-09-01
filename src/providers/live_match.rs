//! Contexto mínimo y de solo lectura de la partida en curso.

use std::{collections::HashMap, time::Duration};

use reqwest::{StatusCode, blocking::Client};
use serde_json::Value;

use super::{
    ProviderError,
    capabilities::GamePhase,
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
    pub queue: Option<String>,
    pub phase: GamePhase,
    /// PUUID -> PartyID efímero. Solo se usa para producir Grupo A/B o Solo.
    pub party_ids: HashMap<String, String>,
}

pub(crate) struct LiveMatchSource {
    client: Client,
    stats: RosterStatsSource,
    pd_base_url: Option<String>,
}

impl LiveMatchSource {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("cliente live"),
            stats: RosterStatsSource::new(),
            pd_base_url: None,
        }
    }

    pub(crate) fn fetch_with_party_lookup(
        &self,
        request: &LiveMatchRequest,
        lookup: impl FnOnce(&[String]) -> HashMap<String, String>,
    ) -> Result<LiveMatchContext, ProviderError> {
        let payload = self.fetch_payload(request)?;
        let players = roster_players(&payload, &request.own_puuid);
        let subjects = roster_subjects(&players);
        let mut party_ids = request.party_ids.clone();
        party_ids.extend(lookup(&subjects));
        let names = self
            .fetch_visible_names(request, &players, &party_ids)
            .unwrap_or_default();
        let stats_request = RosterStatsRequest {
            shard: request.shard.clone(),
            client_version: request.client_version.clone(),
            access_token: request.access_token.clone(),
            entitlement_token: request.entitlement_token.clone(),
        };
        let queue = request.queue.clone();
        let enrichment = self.stats.fetch(&stats_request, &subjects);
        let observed_counts = party_ids
            .values()
            .fold(HashMap::new(), |mut counts, party| {
                *counts.entry(party.clone()).or_insert(0_usize) += 1;
                counts
            });
        for (subject, inferred_party) in enrichment.inferred_parties {
            let observed_group = party_ids
                .get(&subject)
                .and_then(|party| observed_counts.get(party))
                .is_some_and(|count| *count > 1);
            if !observed_group {
                party_ids.insert(subject, inferred_party);
            }
        }
        parse_live_match_with_names_and_stats(
            &payload,
            &request.own_puuid,
            &names,
            &enrichment.stats,
            queue.as_deref(),
            request.phase,
            &party_ids,
        )
    }

    /// Reconsulta únicamente la relación jugador/grupo. Presence puede tardar
    /// varios segundos en publicar los grupos enemigos después de formar el roster.
    pub(crate) fn fetch_party_update(
        &self,
        request: &LiveMatchRequest,
        lookup: impl FnOnce(&[String]) -> HashMap<String, String>,
    ) -> Result<LivePartyUpdate, ProviderError> {
        let payload = self.fetch_payload(request)?;
        let players = roster_players(&payload, &request.own_puuid);
        let subjects = roster_subjects(&players);
        let mut party_ids = request.party_ids.clone();
        party_ids.extend(lookup(&subjects));
        Ok(party_update(&players, &party_ids))
    }

    fn fetch_payload(&self, request: &LiveMatchRequest) -> Result<Value, ProviderError> {
        let endpoint = if matches!(request.phase, GamePhase::PreGame | GamePhase::AgentSelect) {
            "pregame"
        } else {
            "core-game"
        };
        let url = format!(
            "https://glz-{}-1.{}.a.pvp.net/{endpoint}/v1/matches/{}",
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
            status if status.is_success() => response
                .json::<Value>()
                .map_err(|_| ProviderError::Parse("JSON inválido en partida actual".into())),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(ProviderError::Unauthorized("GLZ rechazó la sesión".into()))
            }
            status => Err(ProviderError::Unavailable(format!(
                "GLZ respondió HTTP {status}"
            ))),
        }
    }

    /// Una sola resolución por partida. Las identidades ocultas se excluyen,
    /// salvo la cuenta local y quienes comparten su party: el propio cliente
    /// ya presenta esos nombres al usuario que formó el grupo.
    fn fetch_visible_names(
        &self,
        request: &LiveMatchRequest,
        players: &[Value],
        party_ids: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, ProviderError> {
        let subjects = players
            .iter()
            .filter(|player| {
                let subject = player
                    .get("Subject")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let hidden = player
                    .pointer("/PlayerIdentity/Incognito")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                !hidden
                    || subject == request.own_puuid.as_str()
                    || same_own_party(party_ids, &request.own_puuid, subject)
            })
            .filter_map(|player| player.get("Subject").and_then(Value::as_str))
            .filter(|subject| !subject.is_empty())
            .collect::<Vec<_>>();
        if subjects.is_empty() {
            return Ok(HashMap::new());
        }
        let base = self
            .pd_base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/name-service/v2/players");
        let response = self
            .client
            .put(url)
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

/// Resultado seguro de una actualización de grupos. No expone PUUID ni PartyID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LivePartyUpdate {
    pub premades: Vec<DataAvailability<String>>,
    pub complete: bool,
}

#[cfg(test)]
pub(crate) fn parse_live_match(
    payload: &Value,
    own_puuid: &str,
) -> Result<LiveMatchContext, ProviderError> {
    parse_live_match_with_names_and_stats(
        payload,
        own_puuid,
        &HashMap::new(),
        &HashMap::new(),
        None,
        GamePhase::InMatch,
        &HashMap::new(),
    )
}

fn parse_live_match_with_names_and_stats(
    payload: &Value,
    own_puuid: &str,
    names: &HashMap<String, String>,
    stats: &HashMap<String, HistoricalStats>,
    queue: Option<&str>,
    phase: GamePhase,
    party_ids: &HashMap<String, String>,
) -> Result<LiveMatchContext, ProviderError> {
    let object = payload
        .as_object()
        .ok_or_else(|| ProviderError::Parse("partida actual inválida".into()))?;
    let payload_queue = object
        .get("QueueID")
        .or_else(|| object.get("QueueId"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let mode = display_mode(
        &required_asset_any(object, &["ModeID", "Mode", "QueueID"])?,
        queue.or(payload_queue),
    );
    let map = required_asset(object, "MapID")?;
    let players = roster_players(payload, own_puuid);
    let agent = players
        .iter()
        .find(|player| player.get("Subject").and_then(Value::as_str) == Some(own_puuid))
        .and_then(|player| player.get("CharacterID"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(agent_label);
    let roster = if players.is_empty() {
        None
    } else {
        Some(normalize_roster(
            &players,
            own_puuid,
            names,
            stats,
            matches!(phase, GamePhase::PreGame | GamePhase::AgentSelect),
            party_ids,
        )?)
    };
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
    force_allies: bool,
    party_ids: &HashMap<String, String>,
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
    let free_for_all = !force_allies && (own_team.is_empty() || distinct_teams.len() < 2);
    let premades = normalized_premades(players, party_ids);
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
            let (side, slot) = if force_allies {
                ally_slot = ally_slot.saturating_add(1);
                (RosterSide::Ally, ally_slot)
            } else if free_for_all {
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
            let own_party_member = same_own_party(party_ids, own_puuid, subject);
            let identity = if hidden && !is_self && !own_party_member {
                DataAvailability::Hidden
            } else {
                names
                    .get(subject)
                    .cloned()
                    .map(DataAvailability::Available)
                    .unwrap_or_else(|| {
                        if is_self {
                            DataAvailability::Available("Tú".into())
                        } else {
                            DataAvailability::NotAvailable
                        }
                    })
            };
            let agent = player
                .get("CharacterID")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(agent_label)
                .filter(|value| value != "no disponible")
                .map(DataAvailability::Available)
                .unwrap_or(DataAvailability::NotAvailable);
            let rank_tier = player
                .get("CompetitiveTier")
                .or_else(|| player.pointer("/PlayerIdentity/CompetitiveTier"))
                .or_else(|| player.pointer("/SeasonalBadgeInfo/Rank"))
                .and_then(Value::as_u64)
                .filter(|tier| *tier > 0)
                .or_else(|| stats.get(subject).and_then(|stats| stats.competitive_tier));
            let rank = rank_tier
                .and_then(competitive_tier_label)
                .map(DataAvailability::Available)
                .unwrap_or(DataAvailability::NotAvailable);
            let hide_level = player
                .pointer("/PlayerIdentity/HideAccountLevel")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let level = if hide_level && !is_self && !own_party_member {
                DataAvailability::Hidden
            } else {
                player
                    .pointer("/PlayerIdentity/AccountLevel")
                    .and_then(Value::as_u64)
                    .and_then(|level| u32::try_from(level).ok())
                    .filter(|level| *level > 0)
                    .or_else(|| stats.get(subject).and_then(|stats| stats.account_level))
                    .map(DataAvailability::Available)
                    .unwrap_or(DataAvailability::NotAvailable)
            };
            RosterPlayer {
                side,
                slot,
                is_self,
                identity,
                agent,
                rank,
                level,
                premade: premades
                    .get(subject)
                    .cloned()
                    .map(DataAvailability::Available)
                    .unwrap_or(DataAvailability::NotAvailable),
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

fn same_own_party(party_ids: &HashMap<String, String>, own_puuid: &str, subject: &str) -> bool {
    if subject.is_empty() || subject == own_puuid {
        return false;
    }
    party_ids
        .get(own_puuid)
        .zip(party_ids.get(subject))
        .is_some_and(|(own_party, other_party)| own_party == other_party)
}

/// Current Game usa `Players`; Agent Select expone únicamente el equipo aliado
/// en `AllyTeam` (o dentro de `Teams`, según la versión del cliente).
fn roster_players(payload: &Value, own_puuid: &str) -> Vec<Value> {
    if let Some(players) = payload.get("Players").and_then(Value::as_array) {
        return players.clone();
    }
    if let Some(players) = payload
        .pointer("/AllyTeam/Players")
        .and_then(Value::as_array)
    {
        return players.clone();
    }
    payload
        .get("Teams")
        .and_then(Value::as_array)
        .and_then(|teams| {
            teams.iter().find_map(|team| {
                let players = team.get("Players").and_then(Value::as_array)?;
                players
                    .iter()
                    .any(|player| player.get("Subject").and_then(Value::as_str) == Some(own_puuid))
                    .then_some(players)
            })
        })
        .cloned()
        .unwrap_or_default()
}

fn normalized_premades(
    players: &[Value],
    party_ids: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut counts = HashMap::<String, usize>::new();
    for player in players {
        let Some(subject) = player.get("Subject").and_then(Value::as_str) else {
            continue;
        };
        if let Some(party) = party_ids.get(subject) {
            *counts.entry(party.clone()).or_default() += 1;
        }
    }

    let mut labels = HashMap::<String, String>::new();
    let mut next = b'A';
    let mut result = HashMap::new();
    for player in players {
        let Some(subject) = player.get("Subject").and_then(Value::as_str) else {
            continue;
        };
        let Some(party) = party_ids.get(subject) else {
            continue;
        };
        let label = if counts.get(party).copied().unwrap_or(0) > 1 {
            labels
                .entry(party.clone())
                .or_insert_with(|| {
                    let label = format!("Grupo {}", char::from(next));
                    next = next.saturating_add(1);
                    label
                })
                .clone()
        } else {
            "Solo".into()
        };
        result.insert(subject.to_owned(), label);
    }
    result
}

fn party_update(players: &[Value], party_ids: &HashMap<String, String>) -> LivePartyUpdate {
    let premades = normalized_premades(players, party_ids);
    let subjects = roster_subjects(players);
    LivePartyUpdate {
        premades: players
            .iter()
            .map(|player| {
                let subject = player
                    .get("Subject")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                premades
                    .get(subject)
                    .cloned()
                    .map(DataAvailability::Available)
                    .unwrap_or(DataAvailability::NotAvailable)
            })
            .collect(),
        complete: !subjects.is_empty()
            && subjects
                .iter()
                .all(|subject| party_ids.contains_key(subject)),
    }
}

fn display_mode(mode: &str, queue: Option<&str>) -> String {
    match queue.map(str::to_ascii_lowercase).as_deref() {
        Some("competitive") => "Competitivo".into(),
        Some("unrated") | Some("standard") => "Normal".into(),
        Some("swiftplay") => "Swiftplay".into(),
        Some("spikerush") => "Spike Rush".into(),
        Some("deathmatch") => "Deathmatch".into(),
        Some("teamdeathmatch") => "Team Deathmatch".into(),
        Some("escalation") => "Escalation".into(),
        _ if mode.eq_ignore_ascii_case("bomb") => "Estándar".into(),
        _ => mode.to_owned(),
    }
}

fn roster_subjects(players: &[Value]) -> Vec<String> {
    players
        .iter()
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

fn required_asset_any(
    object: &serde_json::Map<String, Value>,
    fields: &[&str],
) -> Result<String, ProviderError> {
    fields
        .iter()
        .find_map(|field| {
            object
                .get(*field)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .map(asset_label)
        .ok_or_else(|| ProviderError::Parse(format!("{} ausente en partida actual", fields[0])))
}

/// Las rutas internas de Riot suelen terminar en un nombre legible. Los UUIDs
/// de los agentes se resuelven con el catálogo integrado; no implica otra
/// consulta de red durante una partida.
pub(crate) fn asset_label(value: &str) -> String {
    let parts = value.trim_matches('/').split('/').collect::<Vec<_>>();
    let candidate = parts.last().copied().unwrap_or(value);
    if let Some(mode) = internal_mode_name(candidate) {
        return mode.into();
    }
    if candidate
        .chars()
        .all(|character| character.is_ascii_hexdigit() || character == '-')
    {
        "no disponible".into()
    } else {
        map_name(candidate).unwrap_or(candidate).to_owned()
    }
}

fn internal_mode_name(candidate: &str) -> Option<&'static str> {
    let candidate = candidate.to_ascii_lowercase();
    if candidate.contains("bombgamemode") {
        Some("Bomb")
    } else if candidate.contains("teamdeathmatch") || candidate.contains("hurm") {
        Some("TeamDeathmatch")
    } else if candidate.contains("deathmatch") {
        Some("Deathmatch")
    } else if candidate.contains("swiftplay") {
        Some("Swiftplay")
    } else if candidate.contains("spikerush") || candidate.contains("quickbomb") {
        Some("SpikeRush")
    } else if candidate.contains("escalation") || candidate.contains("gunprogression") {
        Some("Escalation")
    } else {
        None
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
            ("enemy-two".to_owned(), "RivalDos#BR".to_owned()),
            ("hidden-secret".to_owned(), "No debe aparecer".to_owned()),
        ]);
        let stats = HashMap::from([(
            "hidden-secret".to_owned(),
            HistoricalStats {
                matches: 2,
                competitive_tier: Some(18),
                account_level: Some(77),
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
                    {"Subject":"me", "TeamID":"Blue", "CharacterID":"8e253930-4c05-31dd-1b6c-968525494517", "CompetitiveTier":18, "PlayerIdentity":{"AccountLevel":142}},
                    {"Subject":"ally", "TeamID":"Blue", "CharacterID":"320b2a48-4d9b-a075-30f1-1f93a9b638fa", "CompetitiveTier":12, "PlayerIdentity":{"AccountLevel":90,"Incognito":true,"HideAccountLevel":true}},
                    {"Subject":"enemy", "TeamID":"Red", "CharacterID":"add6443a-41bd-e414-f6ad-e58d267f4e95", "CompetitiveTier":27},
                    {"Subject":"enemy-two", "TeamID":"Red", "CharacterID":"320b2a48-4d9b-a075-30f1-1f93a9b638fa", "CompetitiveTier":21},
                    {"Subject":"hidden-secret", "TeamID":"Red", "CharacterID":"569fdd95-4d10-43ab-ca70-79becc718b46", "PlayerIdentity":{"Incognito":true}}
                ]
            }),
            "me",
            &names,
            &stats,
            Some("competitive"),
            GamePhase::InMatch,
            &HashMap::from([
                ("me".into(), "party-a".into()),
                ("ally".into(), "party-a".into()),
                ("enemy".into(), "party-b".into()),
                ("enemy-two".into(), "party-b".into()),
                ("hidden-secret".into(), "party-c".into()),
            ]),
        )
        .unwrap();
        let roster = context.roster.unwrap();

        assert_eq!(roster.allies().count(), 2);
        assert_eq!(roster.enemies().count(), 3);
        assert_eq!(
            roster.players[1].identity,
            DataAvailability::Available("Aliado#LAN".into())
        );
        assert_eq!(
            roster.players[2].rank,
            DataAvailability::Available("Radiante".into())
        );
        assert_eq!(roster.players[4].identity, DataAvailability::Hidden);
        assert_eq!(roster.players[0].level, DataAvailability::Available(142));
        assert_eq!(roster.players[1].level, DataAvailability::Available(90));
        assert_eq!(roster.players[4].level, DataAvailability::Available(77));
        assert_eq!(
            roster.players[1].premade,
            DataAvailability::Available("Grupo A".into())
        );
        assert_eq!(
            roster.players[2].premade,
            DataAvailability::Available("Grupo B".into())
        );
        assert_eq!(
            roster.players[3].premade,
            DataAvailability::Available("Grupo B".into())
        );
        assert_eq!(
            roster.players[4].rank,
            DataAvailability::Available("Diamante 1".into())
        );
        assert_eq!(context.mode, "Competitivo");
        assert!(matches!(
            &roster.players[4].stats,
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
    fn normalizes_agent_select_teammates_levels_and_premades() {
        let names = HashMap::from([
            ("me".to_owned(), "Norte#LAS".to_owned()),
            ("ally".to_owned(), "Aliado#LAN".to_owned()),
        ]);
        let context = parse_live_match_with_names_and_stats(
            &serde_json::json!({
                "Mode":"/Game/GameModes/Bomb/BombGameMode.BombGameMode_C",
                "MapID":"/Game/Maps/Ascent/Ascent",
                "AllyTeam":{"Players":[
                    {"Subject":"me", "CharacterID":"8e253930-4c05-31dd-1b6c-968525494517", "CompetitiveTier":21, "PlayerIdentity":{"AccountLevel":142}},
                    {"Subject":"ally", "CharacterID":"", "CompetitiveTier":18, "PlayerIdentity":{"AccountLevel":90,"HideAccountLevel":true}}
                ]}
            }),
            "me",
            &names,
            &HashMap::new(),
            Some("competitive"),
            GamePhase::AgentSelect,
            &HashMap::from([
                ("me".into(), "party-a".into()),
                ("ally".into(), "party-a".into()),
            ]),
        )
        .unwrap();
        let roster = context.roster.unwrap();

        assert_eq!(context.mode, "Competitivo");
        assert_eq!(context.agent.as_deref(), Some("Omen"));
        assert_eq!(roster.allies().count(), 2);
        assert_eq!(roster.participants().count(), 0);
        assert_eq!(roster.players[0].level, DataAvailability::Available(142));
        assert_eq!(roster.players[1].level, DataAvailability::Available(90));
        assert_eq!(
            roster.players[0].premade,
            DataAvailability::Available("Grupo A".into())
        );
        assert_eq!(roster.players[1].agent, DataAvailability::NotAvailable);
    }

    #[test]
    fn normalizes_a_full_five_stack_as_one_premade() {
        let players = (1..=5)
            .map(|slot| {
                serde_json::json!({
                    "Subject": format!("ally-{slot}"),
                    "TeamID": "Blue",
                    "PlayerIdentity": {"AccountLevel": 100 + slot}
                })
            })
            .collect::<Vec<_>>();
        let parties = (1..=5)
            .map(|slot| (format!("ally-{slot}"), "five-stack".to_owned()))
            .collect::<HashMap<_, _>>();

        let roster = normalize_roster(
            &players,
            "ally-1",
            &HashMap::new(),
            &HashMap::new(),
            true,
            &parties,
        )
        .unwrap();

        assert_eq!(roster.allies().count(), 5);
        assert!(
            roster.players.iter().all(|player| {
                player.premade == DataAvailability::Available("Grupo A".to_owned())
            })
        );
    }

    #[test]
    fn progressive_party_update_recovers_enemy_premades() {
        let players = (1..=10)
            .map(|slot| serde_json::json!({"Subject": format!("player-{slot}")}))
            .collect::<Vec<_>>();
        let partial = HashMap::from([
            ("player-1".into(), "own".into()),
            ("player-2".into(), "own".into()),
        ]);
        assert!(!party_update(&players, &partial).complete);

        let complete = HashMap::from([
            ("player-1".into(), "own".into()),
            ("player-2".into(), "own".into()),
            ("player-3".into(), "solo-3".into()),
            ("player-4".into(), "solo-4".into()),
            ("player-5".into(), "solo-5".into()),
            ("player-6".into(), "enemy-a".into()),
            ("player-7".into(), "enemy-a".into()),
            ("player-8".into(), "enemy-a".into()),
            ("player-9".into(), "enemy-b".into()),
            ("player-10".into(), "enemy-b".into()),
        ]);
        let update = party_update(&players, &complete);

        assert!(update.complete);
        assert_eq!(
            update.premades[5],
            DataAvailability::Available("Grupo B".into())
        );
        assert_eq!(update.premades[6], update.premades[5]);
        assert_eq!(update.premades[7], update.premades[5]);
        assert_eq!(
            update.premades[8],
            DataAvailability::Available("Grupo C".into())
        );
        assert_eq!(update.premades[9], update.premades[8]);
    }

    #[test]
    fn resolves_internal_map_names_to_public_labels() {
        assert_eq!(asset_label("/Game/Maps/Juliett/Juliett"), "Sunset");
        assert_eq!(asset_label("/Game/Maps/Triad/Triad"), "Haven");
        assert_eq!(asset_label("/Game/Maps/Plummet/Plummet"), "Summit");
    }

    #[test]
    fn resolves_runtime_mode_class_names_to_stable_labels() {
        assert_eq!(
            asset_label("/Game/GameModes/Bomb/BombGameMode.BombGameMode_C"),
            "Bomb"
        );
        assert_eq!(
            asset_label("/Game/GameModes/Deathmatch/DeathmatchGameMode_C"),
            "Deathmatch"
        );
    }

    #[test]
    fn translates_verified_queues_and_keeps_bomb_fallback_honest() {
        assert_eq!(display_mode("Bomb", Some("competitive")), "Competitivo");
        assert_eq!(display_mode("Bomb", Some("unrated")), "Normal");
        assert_eq!(display_mode("Bomb", None), "Estándar");
    }

    #[test]
    fn uses_put_to_resolve_only_visible_names() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let size = stream.read(&mut chunk).unwrap();
                if size == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..size]);
                let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + content_length {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&bytes);
            let lower = request.to_ascii_lowercase();
            assert!(lower.starts_with("put /name-service/v2/players http/1.1"));
            assert!(lower.contains("authorization: bearer access"));
            assert!(request.contains("visible"));
            assert!(request.contains("hidden-party"));
            assert!(!request.contains("hidden-enemy"));
            let body = r#"[
                {"Subject":"visible","GameName":"Nombre","TagLine":"LAS"},
                {"Subject":"hidden-party","GameName":"Amigo","TagLine":"LAS"}
            ]"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let mut source = LiveMatchSource::new();
        source.pd_base_url = Some(format!("http://{address}"));
        let request = LiveMatchRequest {
            match_id: "match".into(),
            region: "latam".into(),
            shard: "na".into(),
            client_version: "version".into(),
            access_token: "access".into(),
            entitlement_token: "entitlement".into(),
            own_puuid: "me".into(),
            queue: Some("competitive".into()),
            phase: GamePhase::InMatch,
            party_ids: HashMap::from([
                ("me".into(), "party-a".into()),
                ("hidden-party".into(), "party-a".into()),
                ("hidden-enemy".into(), "party-b".into()),
            ]),
        };
        let players = serde_json::json!([
            {"Subject":"visible", "PlayerIdentity":{"Incognito":false}},
            {"Subject":"hidden-party", "PlayerIdentity":{"Incognito":true}},
            {"Subject":"hidden-enemy", "PlayerIdentity":{"Incognito":true}}
        ]);
        let names = source
            .fetch_visible_names(&request, players.as_array().unwrap(), &request.party_ids)
            .unwrap();

        assert_eq!(names.get("visible").map(String::as_str), Some("Nombre#LAS"));
        assert_eq!(
            names.get("hidden-party").map(String::as_str),
            Some("Amigo#LAS")
        );
        assert!(!names.contains_key("hidden-enemy"));
        server.join().unwrap();
    }
}
