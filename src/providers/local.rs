//! Proveedor de la Local Client API de Riot.
//!
//! Solo conversa con `127.0.0.1` y conserva la contraseña del lockfile en
//! memoria durante cada solicitud. Las fases finas por WebSocket se añadirán
//! cuando se valide su contrato con un cliente real.

use std::{net::TcpStream, path::PathBuf, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use native_tls::TlsConnector;
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
};
use serde_json::Value;
use tungstenite::{Message, client::IntoClientRequest};

use crate::{
    game::{self, GameState},
    providers::{
        capabilities::{Confidence, GamePhase, GameStateSource, ProviderError, StateInfo},
        lockfile::{self, Lockfile, LockfileError},
    },
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const HELP_ENDPOINT: &str = "/help";
const ENTITLEMENTS_ENDPOINT: &str = "/entitlements/v1/token";
const EXTERNAL_SESSIONS_ENDPOINT: &str = "/product-session/v1/external-sessions";
const REGION_LOCALE_ENDPOINT: &str = "/riotclient/region-locale";
const WAMP_SUBSCRIBE: u8 = 5;
#[allow(dead_code)] // El stream continuo se conecta al watcher en la siguiente iteración.
const WAMP_EVENT: u8 = 8;
const JSON_API_EVENT_TOPIC: &str = "OnJsonApiEvent";

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Modelo listo para el stream continuo de eventos.
pub(crate) struct LocalWsEvent {
    pub topic: String,
    pub uri: String,
    pub event_type: Option<String>,
}

/// Tokens locales que nunca se serializan, imprimen ni persisten.
pub(crate) struct LocalTokens {
    pub access_token: String,
    pub entitlement_token: String,
}

impl std::fmt::Debug for LocalTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalTokens")
            .field("access_token", &"<redacted>")
            .field("entitlement_token", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LocalApiInfo {
    pub region: Option<String>,
    pub locale: Option<String>,
    pub entitlements_available: bool,
    pub external_sessions_available: bool,
}

/// Fuente de estado basada en el servidor local que abre Riot Client.
pub struct LocalClientSource {
    client: Client,
    lockfile_path: Option<PathBuf>,
}

impl LocalClientSource {
    pub fn new() -> Self {
        Self::with_lockfile_path(None)
    }

    fn with_lockfile_path(lockfile_path: Option<PathBuf>) -> Self {
        // El certificado autofirmado se tolera exclusivamente porque la URL se
        // compone internamente como 127.0.0.1 a partir del lockfile.
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .danger_accept_invalid_certs(true)
            .build()
            .expect("el cliente HTTP local debe poder construirse");
        Self {
            client,
            lockfile_path,
        }
    }

    fn read_lockfile(&self) -> Result<Lockfile, LockfileError> {
        match &self.lockfile_path {
            Some(path) => lockfile::read(path),
            None => lockfile::read_default(),
        }
    }

    fn check_health(&self, lockfile: &Lockfile) -> Result<(), ProviderError> {
        self.request(lockfile, HELP_ENDPOINT).map(|_| ())
    }

    pub(crate) fn inspect_api(&self) -> Result<LocalApiInfo, ProviderError> {
        let lockfile = self
            .read_lockfile()
            .map_err(|error| ProviderError::NotConfigured(error.to_string()))?;
        self.check_health(&lockfile)?;
        let tokens = self.tokens_from(&lockfile)?;
        self.json_from(&lockfile, EXTERNAL_SESSIONS_ENDPOINT)?;
        let locale = self.json_from(&lockfile, REGION_LOCALE_ENDPOINT)?;
        Ok(LocalApiInfo {
            region: optional_text(&locale, "region"),
            locale: optional_text(&locale, "locale"),
            entitlements_available: !tokens.access_token.is_empty()
                && !tokens.entitlement_token.is_empty(),
            external_sessions_available: true,
        })
    }

    fn tokens_from(&self, lockfile: &Lockfile) -> Result<LocalTokens, ProviderError> {
        let value = self.json_from(lockfile, ENTITLEMENTS_ENDPOINT)?;
        Ok(LocalTokens {
            access_token: required_text(&value, "accessToken")?,
            entitlement_token: required_text(&value, "token")?,
        })
    }

    fn json_from(&self, lockfile: &Lockfile, endpoint: &str) -> Result<Value, ProviderError> {
        self.request(lockfile, endpoint)?
            .json::<Value>()
            .map_err(|_| ProviderError::Parse(format!("JSON inválido en {endpoint}")))
    }

    fn request(&self, lockfile: &Lockfile, endpoint: &str) -> Result<Response, ProviderError> {
        let url = format!(
            "{}://127.0.0.1:{}{endpoint}",
            lockfile.protocol, lockfile.port
        );
        let response = self
            .client
            .get(url)
            .basic_auth("riot", Some(lockfile.password()))
            .send()
            .map_err(|error| ProviderError::Network(error.to_string()))?;

        match response.status() {
            status if status.is_success() => Ok(()),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
                "la Local Client API rechazó las credenciales del lockfile".into(),
            )),
            status => Err(ProviderError::Unavailable(format!(
                "la Local Client API respondió HTTP {status}"
            ))),
        }
        .map(|_| response)
    }

    /// Verifica el handshake WAMP y registra la suscripción sin consumir eventos.
    pub(crate) fn validate_websocket(&self) -> Result<(), ProviderError> {
        let lockfile = self
            .read_lockfile()
            .map_err(|error| ProviderError::NotConfigured(error.to_string()))?;
        let stream = TcpStream::connect(("127.0.0.1", lockfile.port))
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        stream
            .set_read_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        stream
            .set_write_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let tls = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let stream = tls
            .connect("127.0.0.1", stream)
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let mut request = format!("wss://127.0.0.1:{}", lockfile.port)
            .into_client_request()
            .map_err(|error| ProviderError::Parse(error.to_string()))?;
        let authorization = format!(
            "Basic {}",
            STANDARD.encode(format!("riot:{}", lockfile.password()))
        );
        request.headers_mut().insert(
            "Authorization",
            authorization
                .parse()
                .expect("Basic Auth es un header válido"),
        );
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            "wamp".parse().expect("wamp es un header válido"),
        );
        let (mut socket, _) = tungstenite::client(request, stream)
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let subscription = serde_json::json!([WAMP_SUBSCRIBE, JSON_API_EVENT_TOPIC]).to_string();
        socket
            .send(Message::Text(subscription.into()))
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let _ = socket.close(None);
        Ok(())
    }
}

fn required_text(value: &Value, field: &str) -> Result<String, ProviderError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ProviderError::Parse(format!("campo `{field}` ausente en respuesta local")))
}

fn optional_text(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[allow(dead_code)] // Se consume al integrar el bucle de eventos con Watcher.
pub(crate) fn parse_wamp_event(payload: &str) -> Option<LocalWsEvent> {
    let message = serde_json::from_str::<Value>(payload).ok()?;
    let values = message.as_array()?;
    if values.first()?.as_u64()? != u64::from(WAMP_EVENT) {
        return None;
    }
    let topic = values.get(1)?.as_str()?.to_owned();
    let event = values.get(2)?.as_object()?;
    Some(LocalWsEvent {
        topic,
        uri: event.get("uri")?.as_str()?.to_owned(),
        event_type: event
            .get("eventType")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

impl Default for LocalClientSource {
    fn default() -> Self {
        Self::new()
    }
}

impl GameStateSource for LocalClientSource {
    fn name(&self) -> &'static str {
        "local-client"
    }

    fn fetch(&self) -> Result<StateInfo, ProviderError> {
        let lockfile = match self.read_lockfile() {
            Ok(lockfile) => lockfile,
            Err(LockfileError::NotFound(_)) => {
                return Ok(StateInfo::new(
                    GamePhase::ClientClosed,
                    GameState::ClientClosed,
                    Confidence::High,
                    self.name(),
                    false,
                    false,
                ));
            }
            Err(error) => return Err(ProviderError::NotConfigured(error.to_string())),
        };

        self.check_health(&lockfile)?;
        Ok(state_after_health(game::detect().state))
    }
}

fn state_after_health(coarse: GameState) -> StateInfo {
    match coarse {
        GameState::GameOpen => StateInfo::new(
            GamePhase::GameOpen,
            GameState::GameOpen,
            Confidence::Medium,
            "local-client",
            true,
            true,
        ),
        _ => StateInfo::new(
            GamePhase::Idle,
            GameState::Idle,
            Confidence::High,
            "local-client",
            true,
            false,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicU64, Ordering},
        thread,
    };

    use super::*;

    static NEXT_TEST_FILE: AtomicU64 = AtomicU64::new(0);

    fn temp_lockfile(contents: &str) -> PathBuf {
        let id = NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("vtracker-local-{id}.lockfile"));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn health_request_uses_basic_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
            assert!(request.starts_with("get /help http/1.1"));
            assert!(request.contains("authorization: basic cmlvddpzzwnyzxq="));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]")
                .unwrap();
        });
        let path = temp_lockfile(&format!("riot:1:{port}:secret:http"));
        let source = LocalClientSource::with_lockfile_path(Some(path.clone()));

        let lockfile = source.read_lockfile().unwrap();
        source.check_health(&lockfile).unwrap();

        server.join().unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_lockfile_means_client_closed() {
        let path = std::env::temp_dir().join("vtracker-lockfile-that-does-not-exist");
        let source = LocalClientSource::with_lockfile_path(Some(path));

        let result = source.fetch().unwrap();

        assert_eq!(result.phase, GamePhase::ClientClosed);
        assert_eq!(result.coarse, GameState::ClientClosed);
    }

    #[test]
    fn health_keeps_game_open_as_coarse_phase_until_websocket_exists() {
        let result = state_after_health(GameState::GameOpen);
        assert_eq!(result.phase, GamePhase::GameOpen);
        assert_eq!(result.confidence, Confidence::Medium);
    }

    #[test]
    fn local_health_does_not_claim_fine_grained_phase() {
        let result = state_after_health(GameState::Idle);
        assert!(!result.phase.is_fine_grained());
    }

    #[test]
    fn parses_tokens_and_region_without_exposing_values_in_debug() {
        let tokens = LocalTokens {
            access_token: required_text(&serde_json::json!({"accessToken": "a"}), "accessToken")
                .unwrap(),
            entitlement_token: required_text(&serde_json::json!({"token": "b"}), "token").unwrap(),
        };
        let debug = format!("{tokens:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("access_token: \"a\""));
        assert_eq!(
            optional_text(
                &serde_json::json!({"region": "latam", "locale": "es-CL"}),
                "region"
            ),
            Some("latam".into())
        );
    }

    #[test]
    fn parses_only_safe_wamp_event_metadata() {
        let event = parse_wamp_event(
            r#"[8,"OnJsonApiEvent",{"uri":"/riot-messaging-service/v1/message/ares-core-game/core-game/v1/matches/id","eventType":"Update","data":{"secret":"ignored"}}]"#,
        )
        .unwrap();
        assert_eq!(event.topic, "OnJsonApiEvent");
        assert!(event.uri.contains("core-game"));
        assert_eq!(event.event_type.as_deref(), Some("Update"));
        assert!(parse_wamp_event("[0, {}]").is_none());
    }
}
