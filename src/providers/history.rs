//! Historial propio de partidas, obtenido con una única consulta de solo lectura.

use std::time::Duration;

use reqwest::{StatusCode, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProviderError, match_detail::MatchDetailRequest, profile::ProfileRequest};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9";

/// Contexto efímero para el historial del jugador autenticado.
#[derive(Clone)]
pub(crate) struct HistoryRequest {
    pub shard: String,
    pub client_version: String,
    pub access_token: String,
    pub entitlement_token: String,
    pub own_puuid: String,
    pub limit: u8,
}

impl HistoryRequest {
    pub(crate) fn match_detail_request(&self, match_id: String) -> MatchDetailRequest {
        MatchDetailRequest {
            match_id,
            shard: self.shard.clone(),
            client_version: self.client_version.clone(),
            access_token: self.access_token.clone(),
            entitlement_token: self.entitlement_token.clone(),
            own_puuid: self.own_puuid.clone(),
        }
    }

    pub(crate) fn profile_request(&self) -> ProfileRequest {
        ProfileRequest {
            shard: self.shard.clone(),
            client_version: self.client_version.clone(),
            access_token: self.access_token.clone(),
            entitlement_token: self.entitlement_token.clone(),
            own_puuid: self.own_puuid.clone(),
        }
    }
}

impl std::fmt::Debug for HistoryRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryRequest")
            .field("shard", &self.shard)
            .field("client_version", &self.client_version)
            .field("access_token", &"<redacted>")
            .field("entitlement_token", &"<redacted>")
            .field("own_puuid", &"<redacted>")
            .field("limit", &self.limit)
            .finish()
    }
}

/// Entrada segura para la interfaz; no contiene MatchID.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HistoryEntry {
    pub queue: String,
    pub started_at_ms: u64,
}

/// Referencia efímera para encadenar detalles durante un cálculo de métricas.
/// Nunca se muestra ni se persiste.
#[derive(Debug)]
pub(crate) struct OwnHistoryMatch {
    pub match_id: String,
    pub entry: HistoryEntry,
}

pub(crate) struct HistorySource {
    client: Client,
    base_url: Option<String>,
}

impl HistorySource {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de historial debe poder construirse"),
            base_url: None,
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de historial de prueba debe poder construirse"),
            base_url: Some(base_url),
        }
    }

    pub(crate) fn fetch_own(
        &self,
        request: &HistoryRequest,
    ) -> Result<Vec<HistoryEntry>, ProviderError> {
        self.fetch_own_matches(request)
            .map(|matches| matches.into_iter().map(|entry| entry.entry).collect())
    }

    pub(crate) fn fetch_own_matches(
        &self,
        request: &HistoryRequest,
    ) -> Result<Vec<OwnHistoryMatch>, ProviderError> {
        if !(1..=20).contains(&request.limit) {
            return Err(ProviderError::Parse("límite de historial inválido".into()));
        }
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!(
            "{base}/match-history/v1/history/{}?startIndex=0&endIndex={}&queue=competitive",
            request.own_puuid, request.limit
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
                .map_err(|_| ProviderError::Parse("JSON inválido en match-history".into()))
                .and_then(|payload| parse_own_history(&payload, &request.own_puuid, request.limit)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "PD rechazó las credenciales de sesión".into(),
            )),
            StatusCode::NOT_FOUND => Err(ProviderError::EndpointUnavailable {
                endpoint: "/match-history/v1/history/<redacted>".into(),
                status: StatusCode::NOT_FOUND.as_u16(),
            }),
            status => Err(ProviderError::Unavailable(format!(
                "PD respondió HTTP {status} en match-history"
            ))),
        }
    }
}

impl Default for HistorySource {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_own_history(
    payload: &Value,
    own_puuid: &str,
    limit: u8,
) -> Result<Vec<OwnHistoryMatch>, ProviderError> {
    if payload.get("Subject").and_then(Value::as_str) != Some(own_puuid) {
        return Err(ProviderError::Parse(
            "match-history no corresponde al jugador autenticado".into(),
        ));
    }
    let history = payload
        .get("History")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Parse("History ausente o inválido".into()))?;
    history
        .iter()
        .filter(|entry| {
            entry
                .get("QueueID")
                .and_then(Value::as_str)
                .is_some_and(|queue| queue.eq_ignore_ascii_case("competitive"))
        })
        .take(usize::from(limit))
        .map(|entry| {
            let match_id = entry
                .get("MatchID")
                .and_then(Value::as_str)
                .filter(|value| {
                    !value.is_empty()
                        && value
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '-')
                })
                .map(ToOwned::to_owned)
                .ok_or_else(|| ProviderError::Parse("MatchID inválido en match-history".into()))?;
            let queue = entry
                .get("QueueID")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(queue_label)
                .ok_or_else(|| ProviderError::Parse("QueueID ausente en match-history".into()))?;
            let started_at_ms = entry
                .get("GameStartTime")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    ProviderError::Parse("GameStartTime ausente en match-history".into())
                })?;
            Ok(OwnHistoryMatch {
                match_id,
                entry: HistoryEntry {
                    queue,
                    started_at_ms,
                },
            })
        })
        .collect()
}

fn queue_label(queue: &str) -> String {
    match queue.to_ascii_lowercase().as_str() {
        "competitive" => "competitivo".into(),
        "unrated" | "standard" => "normal".into(),
        "deathmatch" => "deathmatch".into(),
        "teamdeathmatch" => "team deathmatch".into(),
        "swiftplay" => "swiftplay".into(),
        "spikerush" => "spike rush".into(),
        "escalation" => "escalation".into(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    fn request() -> HistoryRequest {
        HistoryRequest {
            shard: "na".into(),
            client_version: "version".into(),
            access_token: "access".into(),
            entitlement_token: "entitlement".into(),
            own_puuid: "me".into(),
            limit: 5,
        }
    }

    #[test]
    fn keeps_only_own_safe_history_fields() {
        let entries = parse_own_history(
            &serde_json::json!({
                "Subject": "me",
                "History": [
                    {"MatchID": "dm-id", "QueueID": "deathmatch", "GameStartTime": 456},
                    {"MatchID": "private-id", "QueueID": "competitive", "GameStartTime": 123}
                ]
            }),
            "me",
            1,
        )
        .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].match_id, "private-id");
        assert_eq!(
            entries[0].entry,
            HistoryEntry {
                queue: "competitivo".into(),
                started_at_ms: 123
            }
        );
    }

    #[test]
    fn rejects_history_for_a_different_player() {
        let error = parse_own_history(
            &serde_json::json!({"Subject": "other", "History": []}),
            "me",
            5,
        )
        .unwrap_err();

        assert!(error.to_string().contains("jugador autenticado"));
    }

    #[test]
    fn source_sends_one_authenticated_history_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
            assert!(
                request.starts_with(
                    "get /match-history/v1/history/me?startindex=0&endindex=5&queue=competitive http/1.1"
                )
            );
            assert!(request.contains("authorization: bearer access"));
            assert!(request.contains("x-riot-entitlements-jwt: entitlement"));
            let body = serde_json::json!({
                "Subject": "me",
                "History": [{"MatchID": "id", "QueueID": "competitive", "GameStartTime": 123}]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let source = HistorySource::with_base_url(format!("http://{address}"));

        let entries = source.fetch_own(&request()).unwrap();

        assert_eq!(entries[0].queue, "competitivo");
        server.join().unwrap();
    }
}
