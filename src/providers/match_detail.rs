//! Normalización de la respuesta post-partida `match-details`.
//!
//! Este módulo no realiza solicitudes HTTP. Convierte un JSON ya obtenido por
//! una fuente futura a los modelos internos y rechaza respuestas incompletas.
#![allow(dead_code)] // Se conecta cuando MatchDetailSource obtenga respuestas post-partida.

use std::{collections::HashMap, time::Duration};

use reqwest::{StatusCode, blocking::Client};
use serde_json::Value;

use crate::{
    models::{GameMode, MatchRounds, PlayerRoundStat, Round, RoundCeremony, RoundResult, Team},
    providers::ProviderError,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9";

/// Credenciales y metadatos efímeros para una única consulta post-partida.
/// Nunca se serializan, se imprimen ni se persisten.
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
    pub rounds: MatchRounds,
    pub own_puuid: String,
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
            .map_err(|error| ProviderError::Network(error.to_string()))?;

        match response.status() {
            status if status.is_success() => response
                .json::<Value>()
                .map_err(|_| parse_error("JSON inválido en match-details"))
                .and_then(|payload| parse_completed_match_details(&payload))
                .map(|rounds| CompletedMatch {
                    rounds,
                    own_puuid: request.own_puuid.clone(),
                }),
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
    if !match_info
        .get("isCompleted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(parse_error("match-details aún no está finalizado"));
    }
    let match_id = required_text(match_info, "matchId")?;
    let mode = parse_game_mode(required_text(match_info, "gameMode")?)?;
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
    match value.to_ascii_lowercase().as_str() {
        "competitive" => Ok(GameMode::Competitive),
        "unrated" | "standard" => Ok(GameMode::Unrated),
        "customgame" | "custom" => Ok(GameMode::Custom),
        "swiftplay" => Ok(GameMode::Swiftplay),
        other => Err(parse_error(&format!(
            "modo sin timeline compatible: {other}"
        ))),
    }
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
            "matchInfo": {"matchId": "match", "gameMode": "Competitive", "isCompleted": true},
            "roundResults": [
                {
                    "roundNum": 0,
                    "roundResult": "Eliminated",
                    "roundCeremony": "CeremonyAce",
                    "winningTeam": "Blue",
                    "playerStats": [
                        {"subject": "me", "kills": [{"killer": "me", "victim": "them"}], "damage": [{"receiver": "them", "damage": 150}], "score": 300},
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

        assert_eq!(completed.rounds.rounds.len(), 2);
        assert_eq!(completed.own_puuid, "me");
        server.join().unwrap();
    }
}
