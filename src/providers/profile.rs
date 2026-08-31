//! Perfil propio mínimo: nivel y experiencia de cuenta.

use std::time::Duration;

use reqwest::{StatusCode, blocking::Client};
use serde_json::Value;

use super::ProviderError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9";

#[derive(Clone)]
pub(crate) struct ProfileRequest {
    pub shard: String,
    pub client_version: String,
    pub access_token: String,
    pub entitlement_token: String,
    pub own_puuid: String,
}

impl std::fmt::Debug for ProfileRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileRequest")
            .field("shard", &self.shard)
            .field("client_version", &self.client_version)
            .field("access_token", &"<redacted>")
            .field("entitlement_token", &"<redacted>")
            .field("own_puuid", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnProfile {
    pub level: u32,
    pub xp: u32,
}

/// Estado competitivo del jugador autenticado. Un perfil sin clasificación
/// competitiva se representa con `None`, no como un rango inventado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompetitiveProfile {
    pub tier: u32,
    pub ranked_rating: u32,
    pub wins: u32,
    pub games: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompetitiveUpdate {
    pub tier_after: u32,
    pub ranked_rating_after: Option<u32>,
    pub rr_earned: i32,
    pub performance_bonus: i32,
}

impl CompetitiveProfile {
    /// Respaldo cuando MMR no enlaza correctamente el acto vigente, pero el
    /// historial competitivo sí entrega el último rango/RR confirmado.
    pub(crate) fn from_latest_update(update: &CompetitiveUpdate) -> Option<Self> {
        let ranked_rating = update.ranked_rating_after?;
        (update.tier_after >= 3).then_some(Self {
            tier: update.tier_after,
            ranked_rating,
            wins: 0,
            games: 0,
        })
    }
}

pub(crate) struct PlayerProfileSource {
    client: Client,
    base_url: Option<String>,
}

impl PlayerProfileSource {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de perfil debe poder construirse"),
            base_url: None,
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de perfil de prueba debe poder construirse"),
            base_url: Some(base_url),
        }
    }

    pub(crate) fn fetch_own(&self, request: &ProfileRequest) -> Result<OwnProfile, ProviderError> {
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/account-xp/v1/players/{}", request.own_puuid);
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
                .map_err(|_| ProviderError::Parse("JSON inválido en account-xp".into()))
                .and_then(|payload| parse_own_profile(&payload, &request.own_puuid)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "PD rechazó las credenciales de sesión".into(),
            )),
            StatusCode::NOT_FOUND => Err(ProviderError::EndpointUnavailable {
                endpoint: "/account-xp/v1/players/<redacted>".into(),
                status: StatusCode::NOT_FOUND.as_u16(),
            }),
            status => Err(ProviderError::Unavailable(format!(
                "PD respondió HTTP {status} en account-xp"
            ))),
        }
    }

    /// Consulta una sola vez el MMR del jugador autenticado. No se usa para
    /// roster, matchmaking ni otros perfiles.
    pub(crate) fn fetch_own_competitive(
        &self,
        request: &ProfileRequest,
    ) -> Result<Option<CompetitiveProfile>, ProviderError> {
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/mmr/v1/players/{}", request.own_puuid);
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
                .map_err(|_| ProviderError::Parse("JSON inválido en MMR".into()))
                .and_then(|payload| parse_own_competitive(&payload, &request.own_puuid)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "PD rechazó las credenciales de sesión".into(),
            )),
            StatusCode::NOT_FOUND => Err(ProviderError::EndpointUnavailable {
                endpoint: "/mmr/v1/players/<redacted>".into(),
                status: StatusCode::NOT_FOUND.as_u16(),
            }),
            status => Err(ProviderError::Unavailable(format!(
                "PD respondió HTTP {status} en MMR"
            ))),
        }
    }

    /// Últimas variaciones competitivas propias. Los MatchID del endpoint se
    /// descartan antes de salir del proveedor.
    pub(crate) fn fetch_own_competitive_updates(
        &self,
        request: &ProfileRequest,
        limit: u8,
    ) -> Result<Vec<CompetitiveUpdate>, ProviderError> {
        if !(1..=20).contains(&limit) {
            return Err(ProviderError::Parse(
                "límite de cambios competitivos inválido".into(),
            ));
        }
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!(
            "{base}/mmr/v1/players/{}/competitiveupdates?startIndex=0&endIndex={limit}&queue=competitive",
            request.own_puuid
        );
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
                .map_err(|_| ProviderError::Parse("JSON inválido en cambios competitivos".into()))
                .and_then(|payload| {
                    parse_own_competitive_updates(&payload, &request.own_puuid, limit)
                }),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "PD rechazó las credenciales de sesión".into(),
            )),
            StatusCode::NOT_FOUND => Err(ProviderError::EndpointUnavailable {
                endpoint: "/mmr/v1/players/<redacted>/competitiveupdates".into(),
                status: StatusCode::NOT_FOUND.as_u16(),
            }),
            status => Err(ProviderError::Unavailable(format!(
                "PD respondió HTTP {status} en cambios competitivos"
            ))),
        }
    }
}

impl Default for PlayerProfileSource {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_own_profile(payload: &Value, own_puuid: &str) -> Result<OwnProfile, ProviderError> {
    if payload.get("Subject").and_then(Value::as_str) != Some(own_puuid) {
        return Err(ProviderError::Parse(
            "account-xp no corresponde al jugador autenticado".into(),
        ));
    }
    let progress = payload
        .get("Progress")
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::Parse("Progress ausente en account-xp".into()))?;
    let level = required_u32(progress, "Level")?;
    let xp = required_u32(progress, "XP")?;
    Ok(OwnProfile { level, xp })
}

fn parse_own_competitive(
    payload: &Value,
    own_puuid: &str,
) -> Result<Option<CompetitiveProfile>, ProviderError> {
    if payload.get("Subject").and_then(Value::as_str) != Some(own_puuid) {
        return Err(ProviderError::Parse(
            "MMR no corresponde al jugador autenticado".into(),
        ));
    }
    let latest = payload
        .get("LatestCompetitiveUpdate")
        .and_then(Value::as_object);
    let season_id = latest
        .and_then(|update| update.get("SeasonID"))
        .and_then(Value::as_str)
        .filter(|season| !season.is_empty());
    let seasonal = season_id.and_then(|season_id| {
        payload
            .get("QueueSkills")
            .and_then(Value::as_object)
            .and_then(|queues| queues.get("competitive"))
            .and_then(Value::as_object)
            .and_then(|queue| queue.get("SeasonalInfoBySeasonID"))
            .and_then(Value::as_object)
            .and_then(|seasons| seasons.get(season_id))
            .and_then(Value::as_object)
    });
    if let Some(seasonal) = seasonal {
        return Ok(Some(CompetitiveProfile {
            tier: required_u32(seasonal, "CompetitiveTier")?,
            ranked_rating: required_u32(seasonal, "RankedRating")?,
            wins: optional_u32(seasonal, "NumberOfWins").unwrap_or(0),
            games: optional_u32(seasonal, "NumberOfGames").unwrap_or(0),
        }));
    }
    let Some(latest) = latest else {
        return Ok(None);
    };
    let Some(tier) = optional_u32(latest, "TierAfterUpdate").filter(|tier| *tier >= 3) else {
        return Ok(None);
    };
    let Some(ranked_rating) = optional_u32(latest, "RankedRatingAfterUpdate") else {
        return Ok(None);
    };
    Ok(Some(CompetitiveProfile {
        tier,
        ranked_rating,
        wins: 0,
        games: 0,
    }))
}

fn parse_own_competitive_updates(
    payload: &Value,
    own_puuid: &str,
    limit: u8,
) -> Result<Vec<CompetitiveUpdate>, ProviderError> {
    if payload.get("Subject").and_then(Value::as_str) != Some(own_puuid) {
        return Err(ProviderError::Parse(
            "cambios competitivos no corresponden al jugador autenticado".into(),
        ));
    }
    let matches = payload
        .get("Matches")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Parse("Matches ausente en cambios competitivos".into()))?;
    matches
        .iter()
        .take(usize::from(limit))
        .map(|entry| {
            let entry = entry
                .as_object()
                .ok_or_else(|| ProviderError::Parse("cambio competitivo inválido".into()))?;
            Ok(CompetitiveUpdate {
                tier_after: required_u32(entry, "TierAfterUpdate")?,
                ranked_rating_after: optional_u32(entry, "RankedRatingAfterUpdate"),
                rr_earned: required_i32(entry, "RankedRatingEarned")?,
                performance_bonus: required_i32(entry, "RankedRatingPerformanceBonus")?,
            })
        })
        .collect()
}

fn optional_u32(object: &serde_json::Map<String, Value>, field: &str) -> Option<u32> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn required_u32(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u32, ProviderError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| ProviderError::Parse(format!("{field} ausente o inválido en account-xp")))
}

fn required_i32(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<i32, ProviderError> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| ProviderError::Parse(format!("{field} ausente o inválido en MMR")))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    fn request() -> ProfileRequest {
        ProfileRequest {
            shard: "na".into(),
            client_version: "version".into(),
            access_token: "access".into(),
            entitlement_token: "entitlement".into(),
            own_puuid: "me".into(),
        }
    }

    #[test]
    fn parses_only_authenticated_profile_fields() {
        let profile = parse_own_profile(
            &serde_json::json!({
                "Subject": "me",
                "Progress": {"Level": 123, "XP": 4567},
                "History": [{"ID": "never kept"}]
            }),
            "me",
        )
        .unwrap();

        assert_eq!(
            profile,
            OwnProfile {
                level: 123,
                xp: 4567
            }
        );
    }

    #[test]
    fn rejects_profile_for_a_different_player() {
        let error = parse_own_profile(
            &serde_json::json!({"Subject": "other", "Progress": {"Level": 1, "XP": 1}}),
            "me",
        )
        .unwrap_err();

        assert!(error.to_string().contains("jugador autenticado"));
    }

    #[test]
    fn parses_only_own_current_competitive_snapshot() {
        let competitive = parse_own_competitive(
            &serde_json::json!({
                "Subject": "me",
                "LatestCompetitiveUpdate": {"SeasonID": "season-current", "MatchID": "discarded"},
                "QueueSkills": {
                    "competitive": {
                        "SeasonalInfoBySeasonID": {
                            "season-old": {"CompetitiveTier": 3, "RankedRating": 0, "NumberOfWins": 1, "NumberOfGames": 2},
                            "season-current": {"CompetitiveTier": 18, "RankedRating": 50, "NumberOfWins": 20, "NumberOfGames": 35}
                        }
                    }
                }
            }),
            "me",
        )
        .unwrap()
        .unwrap();

        assert_eq!(competitive.tier, 18);
        assert_eq!(competitive.ranked_rating, 50);
        assert_eq!(competitive.wins, 20);
        assert_eq!(competitive.games, 35);
    }

    #[test]
    fn accepts_player_without_competitive_history() {
        let competitive = parse_own_competitive(
            &serde_json::json!({"Subject": "me", "LatestCompetitiveUpdate": null}),
            "me",
        )
        .unwrap();

        assert!(competitive.is_none());
    }

    #[test]
    fn parses_own_rr_changes_and_discards_match_ids() {
        let updates = parse_own_competitive_updates(
            &serde_json::json!({
                "Subject": "me",
                "Matches": [
                    {"MatchID": "discarded", "TierAfterUpdate": 18, "RankedRatingAfterUpdate": 64, "RankedRatingEarned": 20, "RankedRatingPerformanceBonus": 3},
                    {"MatchID": "discarded-too", "TierAfterUpdate": 18, "RankedRatingEarned": -17, "RankedRatingPerformanceBonus": 0}
                ]
            }),
            "me",
            5,
        )
        .unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].ranked_rating_after, Some(64));
        assert_eq!(updates[0].rr_earned, 20);
        assert_eq!(updates[1].rr_earned, -17);
    }

    #[test]
    fn falls_back_to_latest_competitive_update_when_season_link_is_missing() {
        let competitive = parse_own_competitive(
            &serde_json::json!({
                "Subject": "me",
                "LatestCompetitiveUpdate": {
                    "TierAfterUpdate": 18,
                    "RankedRatingAfterUpdate": 64
                },
                "QueueSkills": {"competitive": {"SeasonalInfoBySeasonID": {}}}
            }),
            "me",
        )
        .unwrap()
        .unwrap();

        assert_eq!(competitive.tier, 18);
        assert_eq!(competitive.ranked_rating, 64);
        assert_eq!((competitive.wins, competitive.games), (0, 0));
    }

    #[test]
    fn source_sends_one_authenticated_account_xp_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
            assert!(request.starts_with("get /account-xp/v1/players/me http/1.1"));
            assert!(request.contains("authorization: bearer access"));
            let body = serde_json::json!({"Subject": "me", "Progress": {"Level": 10, "XP": 20}})
                .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let source = PlayerProfileSource::with_base_url(format!("http://{address}"));

        let profile = source.fetch_own(&request()).unwrap();

        assert_eq!(profile, OwnProfile { level: 10, xp: 20 });
        server.join().unwrap();
    }

    #[test]
    fn source_sends_one_authenticated_mmr_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
            assert!(request.starts_with("get /mmr/v1/players/me http/1.1"));
            assert!(request.contains("authorization: bearer access"));
            let body = serde_json::json!({
                "Subject": "me",
                "LatestCompetitiveUpdate": {"SeasonID": "season"},
                "QueueSkills": {"competitive": {"SeasonalInfoBySeasonID": {
                    "season": {"CompetitiveTier": 18, "RankedRating": 40, "NumberOfWins": 2, "NumberOfGames": 3}
                }}}
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let source = PlayerProfileSource::with_base_url(format!("http://{address}"));

        let competitive = source.fetch_own_competitive(&request()).unwrap().unwrap();

        assert_eq!(competitive.ranked_rating, 40);
        server.join().unwrap();
    }

    #[test]
    fn source_sends_one_authenticated_competitive_updates_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
            assert!(request.starts_with(
                "get /mmr/v1/players/me/competitiveupdates?startindex=0&endindex=5&queue=competitive http/1.1"
            ));
            let body = serde_json::json!({
                "Subject": "me",
                "Matches": [{"MatchID": "id", "TierAfterUpdate": 18, "RankedRatingEarned": 18, "RankedRatingPerformanceBonus": 0}]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let source = PlayerProfileSource::with_base_url(format!("http://{address}"));

        let updates = source.fetch_own_competitive_updates(&request(), 5).unwrap();

        assert_eq!(updates[0].rr_earned, 18);
        server.join().unwrap();
    }
}
