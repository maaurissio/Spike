//! Proveedor de la Local Client API de Riot.
//!
//! Solo conversa con `127.0.0.1` y conserva la contraseña del lockfile en
//! memoria durante cada solicitud. El WebSocket se usa únicamente para
//! observar metadatos de eventos; sus payloads se descartan.

use std::{
    net::TcpStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD},
};
use native_tls::TlsConnector;
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
};
use serde_json::Value;
use tungstenite::{Message, WebSocket, client::IntoClientRequest};

use crate::{
    game::{self, GameState},
    providers::{
        capabilities::{Confidence, GamePhase, GameStateSource, ProviderError, StateInfo},
        history::HistoryRequest,
        live_match::LiveMatchRequest,
        lockfile::{self, Lockfile, LockfileError},
        match_detail::MatchDetailRequest,
        profile::ProfileRequest,
    },
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const HELP_ENDPOINT: &str = "/help";
const ENTITLEMENTS_ENDPOINT: &str = "/entitlements/v1/token";
const EXTERNAL_SESSIONS_ENDPOINT: &str = "/product-session/v1/external-sessions";
const REGION_LOCALE_ENDPOINT: &str = "/riotclient/region-locale";
const WAMP_SUBSCRIBE: u8 = 5;
const WAMP_EVENT: u8 = 8;
const JSON_API_EVENT_TOPIC: &str = "OnJsonApiEvent";
/// Un evento aislado no debe fijar la interfaz a una fase que ya terminó.
const EVENT_PHASE_TTL: Duration = Duration::from_secs(15);
/// La partida puede permanecer silenciosa en WebSocket, pero el proceso de
/// VALORANT sigue abierto incluso después de volver al menú. Nunca usar el
/// proceso como confirmación indefinida de `InMatch`.
const IN_MATCH_PHASE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalWsEvent {
    pub topic: String,
    pub uri: String,
    pub event_type: Option<String>,
}

impl LocalWsEvent {
    pub(crate) fn phase_hint(&self) -> Option<GamePhase> {
        if self.uri.contains("ares-core-game/core-game/v1/matches/") {
            if self
                .event_type
                .as_deref()
                .is_some_and(|event_type| event_type.eq_ignore_ascii_case("delete"))
            {
                Some(GamePhase::PostMatch)
            } else {
                Some(GamePhase::InMatch)
            }
        } else if self.uri.contains("ares-pregame/pregame/v1/matches/") {
            // La representación de una partida pregame contiene la selección
            // de agente. No se inspecciona ni conserva el contenido del evento.
            Some(GamePhase::AgentSelect)
        } else if self.uri.contains("ares-pregame/pregame/") {
            Some(GamePhase::PreGame)
        } else if self.uri.contains("post-game") {
            Some(GamePhase::PostMatch)
        } else if self.uri.contains("party") {
            Some(GamePhase::Lobby)
        } else {
            None
        }
    }

    pub(crate) fn match_id(&self) -> Option<String> {
        let (_, id) = self.uri.split_once("/matches/")?;
        let id = id.split('/').next()?;
        (!id.is_empty()
            && id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-'))
        .then(|| id.to_owned())
    }
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
    event_phase: Arc<Mutex<Option<EventPhase>>>,
}

#[derive(Clone, Debug)]
struct EventPhase {
    phase: GamePhase,
    observed_at: Instant,
    match_id: Option<String>,
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
            event_phase: Arc::new(Mutex::new(None)),
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

    /// Inicia un listener de solo lectura. Solo guarda fases que se derivan de
    /// una URI conocida; todo evento ambiguo se descarta.
    pub fn start_event_listener(&self) {
        let source = self.clone();
        let _ = thread::Builder::new()
            .name("vtracker-local-events".into())
            .spawn(move || source.listen_for_events());
    }

    fn listen_for_events(self) {
        loop {
            let Ok(mut socket) = self.open_websocket() else {
                self.clear_event_phase();
                thread::sleep(Duration::from_secs(3));
                continue;
            };
            if self.subscribe(&mut socket).is_err() {
                thread::sleep(Duration::from_secs(3));
                continue;
            }
            loop {
                match socket.read() {
                    Ok(Message::Text(payload)) => {
                        if let Some(event) = parse_wamp_event(payload.as_str())
                            && let Some(phase) = event.phase_hint()
                        {
                            self.set_event_phase(phase, event.match_id());
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(error))
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) => {}
                    Err(_) => break,
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn set_event_phase(&self, phase: GamePhase, match_id: Option<String>) {
        if let Ok(mut current) = self.event_phase.lock() {
            let previous_match_id = current.as_ref().and_then(|event| event.match_id.clone());
            *current = Some(EventPhase {
                phase,
                observed_at: Instant::now(),
                match_id: match_id.or(previous_match_id),
            });
        }
    }

    fn clear_event_phase(&self) {
        if let Ok(mut current) = self.event_phase.lock() {
            *current = None;
        }
    }

    fn event_phase(&self, game_running: bool) -> Option<GamePhase> {
        let mut current = self.event_phase.lock().ok()?;
        if current
            .as_ref()
            .is_some_and(|event| event.phase == GamePhase::InMatch)
        {
            if game_running
                && current
                    .as_ref()
                    .is_some_and(|event| event.observed_at.elapsed() <= IN_MATCH_PHASE_TTL)
            {
                return Some(GamePhase::InMatch);
            }
            *current = None;
            return None;
        }
        if current
            .as_ref()
            .is_some_and(|event| event.observed_at.elapsed() > EVENT_PHASE_TTL)
        {
            *current = None;
        }
        current.as_ref().map(|event| event.phase)
    }

    fn event_match_id(&self) -> Option<String> {
        let mut current = self.event_phase.lock().ok()?;
        if current
            .as_ref()
            .is_some_and(|event| event.observed_at.elapsed() > EVENT_PHASE_TTL)
        {
            *current = None;
        }
        current.as_ref().and_then(|event| event.match_id.clone())
    }

    /// Construye el contexto efímero para una única consulta post-partida.
    /// No realiza la consulta remota: esa responsabilidad es de `MatchDetailSource`.
    pub(crate) fn match_detail_request(&self) -> Result<MatchDetailRequest, ProviderError> {
        let match_id = self.event_match_id().ok_or_else(|| {
            ProviderError::NotConfigured("no hay un identificador de partida reciente".into())
        })?;
        let lockfile = self
            .read_lockfile()
            .map_err(|error| ProviderError::NotConfigured(error.to_string()))?;
        self.check_health(&lockfile)?;
        let tokens = self.tokens_from(&lockfile)?;
        let sessions = self.json_from(&lockfile, EXTERNAL_SESSIONS_ENDPOINT)?;
        let session = valorant_session_info(
            &sessions,
            puuid_from_access_token(&tokens.access_token).as_deref(),
        )?;
        Ok(MatchDetailRequest {
            match_id,
            shard: session.shard,
            client_version: session.client_version,
            access_token: tokens.access_token,
            entitlement_token: tokens.entitlement_token,
            own_puuid: session.own_puuid,
        })
    }

    pub(crate) fn live_match_request(&self) -> Result<LiveMatchRequest, ProviderError> {
        let match_id = self
            .event_match_id()
            .ok_or_else(|| ProviderError::NotConfigured("no hay partida reciente".into()))?;
        let lockfile = self
            .read_lockfile()
            .map_err(|error| ProviderError::NotConfigured(error.to_string()))?;
        self.check_health(&lockfile)?;
        let tokens = self.tokens_from(&lockfile)?;
        let sessions = self.json_from(&lockfile, EXTERNAL_SESSIONS_ENDPOINT)?;
        let session = valorant_session_info(
            &sessions,
            puuid_from_access_token(&tokens.access_token).as_deref(),
        )?;
        Ok(LiveMatchRequest {
            match_id,
            region: session.region,
            shard: session.shard,
            client_version: session.client_version,
            access_token: tokens.access_token,
            entitlement_token: tokens.entitlement_token,
            own_puuid: session.own_puuid,
        })
    }

    /// Construye una solicitud acotada para el historial del jugador autenticado.
    /// No ejecuta red remota ni conserva los tokens al retornar.
    pub(crate) fn history_request(&self, limit: u8) -> Result<HistoryRequest, ProviderError> {
        if !(1..=20).contains(&limit) {
            return Err(ProviderError::Parse("límite de historial inválido".into()));
        }
        let lockfile = self
            .read_lockfile()
            .map_err(|error| ProviderError::NotConfigured(error.to_string()))?;
        self.check_health(&lockfile)?;
        let tokens = self.tokens_from(&lockfile)?;
        let sessions = self.json_from(&lockfile, EXTERNAL_SESSIONS_ENDPOINT)?;
        let session = valorant_session_info(
            &sessions,
            puuid_from_access_token(&tokens.access_token).as_deref(),
        )?;
        Ok(HistoryRequest {
            shard: session.shard,
            client_version: session.client_version,
            access_token: tokens.access_token,
            entitlement_token: tokens.entitlement_token,
            own_puuid: session.own_puuid,
            limit,
        })
    }

    /// Construye una solicitud efímera para el perfil del jugador autenticado.
    pub(crate) fn profile_request(&self) -> Result<ProfileRequest, ProviderError> {
        let lockfile = self
            .read_lockfile()
            .map_err(|error| ProviderError::NotConfigured(error.to_string()))?;
        self.check_health(&lockfile)?;
        let tokens = self.tokens_from(&lockfile)?;
        let sessions = self.json_from(&lockfile, EXTERNAL_SESSIONS_ENDPOINT)?;
        let session = valorant_session_info(
            &sessions,
            puuid_from_access_token(&tokens.access_token).as_deref(),
        )?;
        Ok(ProfileRequest {
            shard: session.shard,
            client_version: session.client_version,
            access_token: tokens.access_token,
            entitlement_token: tokens.entitlement_token,
            own_puuid: session.own_puuid,
        })
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
            StatusCode::NOT_FOUND => Err(ProviderError::EndpointUnavailable {
                endpoint: endpoint.into(),
                status: StatusCode::NOT_FOUND.as_u16(),
            }),
            status => Err(ProviderError::Unavailable(format!(
                "la Local Client API respondió HTTP {status} en {endpoint}"
            ))),
        }
        .map(|_| response)
    }

    /// Verifica el handshake WAMP y registra la suscripción sin consumir eventos.
    pub(crate) fn validate_websocket(&self) -> Result<(), ProviderError> {
        let mut socket = self.open_websocket()?;
        self.subscribe(&mut socket)?;
        let _ = socket.close(None);
        Ok(())
    }

    /// Lee solo metadatos de un número acotado de eventos y descarta su payload.
    pub(crate) fn sample_websocket_events(
        &self,
        max_messages: usize,
    ) -> Result<Vec<LocalWsEvent>, ProviderError> {
        let mut socket = self.open_websocket()?;
        self.subscribe(&mut socket)?;
        let mut events = Vec::new();
        for _ in 0..max_messages {
            match socket.read() {
                Ok(Message::Text(payload)) => {
                    if let Some(event) = parse_wamp_event(payload.as_str()) {
                        events.push(event);
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(ProviderError::Network(error.to_string())),
            }
        }
        let _ = socket.close(None);
        Ok(events)
    }

    fn subscribe(
        &self,
        socket: &mut WebSocket<native_tls::TlsStream<TcpStream>>,
    ) -> Result<(), ProviderError> {
        let subscription = serde_json::json!([WAMP_SUBSCRIBE, JSON_API_EVENT_TOPIC]).to_string();
        socket
            .send(Message::Text(subscription.into()))
            .map_err(|error| ProviderError::Network(error.to_string()))
    }

    fn open_websocket(&self) -> Result<WebSocket<native_tls::TlsStream<TcpStream>>, ProviderError> {
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
        let (socket, _) = tungstenite::client(request, stream)
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        Ok(socket)
    }
}

impl Clone for LocalClientSource {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            lockfile_path: self.lockfile_path.clone(),
            event_phase: Arc::clone(&self.event_phase),
        }
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

#[derive(Debug, Eq, PartialEq)]
struct ValorantSessionInfo {
    shard: String,
    region: String,
    client_version: String,
    own_puuid: String,
}

fn valorant_session_info(
    value: &Value,
    token_puuid: Option<&str>,
) -> Result<ValorantSessionInfo, ProviderError> {
    let sessions = value
        .as_object()
        .ok_or_else(|| ProviderError::Parse("sesiones locales inválidas".into()))?;
    let session = sessions
        .values()
        .find(|session| session.get("productId").and_then(Value::as_str) == Some("valorant"))
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::Unavailable("no hay una sesión VALORANT activa".into()))?;
    let client_version = session
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ProviderError::Parse("versión de sesión ausente".into()))?;
    let arguments = session
        .get("launchConfiguration")
        .and_then(Value::as_object)
        .and_then(|configuration| configuration.get("arguments"))
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Parse("argumentos de sesión ausentes".into()))?;
    let shard = arguments
        .iter()
        .filter_map(Value::as_str)
        .find_map(shard_from_argument)
        .ok_or_else(|| ProviderError::Parse("shard no informado por la sesión".into()))?;
    let region = arguments
        .iter()
        .filter_map(Value::as_str)
        .find_map(region_from_argument)
        .ok_or_else(|| ProviderError::Parse("región no informada por la sesión".into()))?;
    let own_puuid = argument_value(
        arguments,
        &["-ares-puuid", "--ares-puuid", "-ares-player-uuid"],
    )
    .or_else(|| {
        ["puuid", "subject", "userId"]
            .iter()
            .find_map(|field| session.get(*field).and_then(Value::as_str))
    })
    .or(token_puuid)
    .filter(|puuid| {
        !puuid.is_empty()
            && puuid
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    })
    .map(ToOwned::to_owned)
    .ok_or_else(|| ProviderError::Parse("puuid no informado por la sesión".into()))?;
    Ok(ValorantSessionInfo {
        shard,
        region,
        client_version,
        own_puuid,
    })
}

/// Obtiene el `sub` del JWT emitido por el cliente local. Es un respaldo para
/// versiones que no incluyen el PUUID en la configuración de lanzamiento.
/// El token, su payload y el PUUID nunca salen de esta función ni se registran.
fn puuid_from_access_token(access_token: &str) -> Option<String> {
    let payload = access_token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("sub")?.as_str().map(ToOwned::to_owned)
}

/// Acepta `-clave=valor` y el formato equivalente `-clave valor`. El valor
/// queda en memoria y nunca se incluye en diagnósticos ni mensajes de error.
fn argument_value<'a>(arguments: &'a [Value], names: &[&str]) -> Option<&'a str> {
    let values = arguments
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    for (index, argument) in values.iter().enumerate() {
        for name in names {
            if let Some(value) = argument.strip_prefix(&format!("{name}=")) {
                return Some(value);
            }
            if argument.eq_ignore_ascii_case(name) {
                return values.get(index + 1).copied();
            }
        }
    }
    None
}

fn region_from_argument(argument: &str) -> Option<String> {
    let value = argument
        .strip_prefix("-ares-deployment=")
        .or_else(|| argument.strip_prefix("-ares-region="))?
        .to_ascii_lowercase();
    match value.as_str() {
        "na" | "latam" | "br" | "eu" | "ap" | "kr" => Some(value),
        "la1" | "la2" => Some("latam".into()),
        _ => None,
    }
}

fn shard_from_argument(argument: &str) -> Option<String> {
    let value = argument
        .strip_prefix("-ares-shard=")
        .or_else(|| argument.strip_prefix("-ares-deployment="))
        .or_else(|| argument.strip_prefix("-ares-region="))?
        .to_ascii_lowercase();
    match value.as_str() {
        "na" | "pbe" | "eu" | "ap" | "kr" => Some(value),
        "latam" | "la1" | "la2" | "br" => Some("na".into()),
        _ => None,
    }
}

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
        let coarse = game::detect().state;
        if let Some(phase) = self.event_phase(coarse == GameState::GameOpen) {
            return Ok(state_from_event_phase(phase));
        }
        Ok(state_after_health(coarse))
    }
}

fn state_from_event_phase(phase: GamePhase) -> StateInfo {
    let (coarse, game_found) = match phase {
        GamePhase::InMatch => (GameState::GameOpen, true),
        GamePhase::Lobby | GamePhase::PreGame | GamePhase::AgentSelect | GamePhase::PostMatch => {
            (GameState::Idle, false)
        }
        _ => return state_after_health(game::detect().state),
    };
    StateInfo::new(
        phase,
        coarse,
        Confidence::High,
        "local-websocket",
        true,
        game_found,
    )
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
    fn missing_endpoint_reports_its_safe_route() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request.starts_with("GET /entitlements/v1/token HTTP/1.1"));
            stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let path = temp_lockfile(&format!("riot:1:{port}:secret:http"));
        let source = LocalClientSource::with_lockfile_path(Some(path.clone()));
        let lockfile = source.read_lockfile().unwrap();

        let error = source.tokens_from(&lockfile).unwrap_err();

        assert!(matches!(
            error,
            ProviderError::EndpointUnavailable { endpoint, status: 404 }
                if endpoint == ENTITLEMENTS_ENDPOINT
        ));
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
        assert_eq!(event.phase_hint(), Some(GamePhase::InMatch));
        assert_eq!(event.match_id().as_deref(), Some("id"));
        assert!(parse_wamp_event("[0, {}]").is_none());
    }

    #[test]
    fn pregame_match_events_map_to_agent_select_without_reading_payload() {
        let event = parse_wamp_event(
            r#"[8,"OnJsonApiEvent",{"uri":"/riot-messaging-service/v1/message/ares-pregame/pregame/v1/matches/id","eventType":"Update","data":{"agent":"ignored"}}]"#,
        )
        .unwrap();

        assert_eq!(event.phase_hint(), Some(GamePhase::AgentSelect));
    }

    #[test]
    fn deleting_current_match_maps_to_postmatch() {
        let event = parse_wamp_event(
            r#"[8,"OnJsonApiEvent",{"uri":"/riot-messaging-service/v1/message/ares-core-game/core-game/v1/matches/id","eventType":"Delete","data":{"ignored":"payload"}}]"#,
        )
        .unwrap();

        assert_eq!(event.phase_hint(), Some(GamePhase::PostMatch));
        assert_eq!(event.match_id().as_deref(), Some("id"));
    }

    #[test]
    fn generic_pregame_events_remain_pregame() {
        let event = parse_wamp_event(
            r#"[8,"OnJsonApiEvent",{"uri":"/riot-messaging-service/v1/message/ares-pregame/pregame/v1/queues","eventType":"Update","data":{}}]"#,
        )
        .unwrap();

        assert_eq!(event.phase_hint(), Some(GamePhase::PreGame));
    }

    #[test]
    fn known_event_phase_is_reported_with_high_confidence() {
        let info = state_from_event_phase(GamePhase::InMatch);
        assert_eq!(info.phase, GamePhase::InMatch);
        assert_eq!(info.coarse, GameState::GameOpen);
        assert_eq!(info.confidence, Confidence::High);
        assert_eq!(info.source, "local-websocket");
    }

    #[test]
    fn ambiguous_events_do_not_change_phase_state() {
        let source = LocalClientSource::with_lockfile_path(Some(PathBuf::from("unused")));
        assert_eq!(source.event_phase(false), None);
        source.set_event_phase(GamePhase::PreGame, None);
        assert_eq!(source.event_phase(false), Some(GamePhase::PreGame));
        source.clear_event_phase();
        assert_eq!(source.event_phase(false), None);
    }

    #[test]
    fn expired_event_phase_is_discarded() {
        let source = LocalClientSource::with_lockfile_path(Some(PathBuf::from("unused")));
        if let Ok(mut phase) = source.event_phase.lock() {
            *phase = Some(EventPhase {
                phase: GamePhase::InMatch,
                observed_at: Instant::now() - EVENT_PHASE_TTL - Duration::from_secs(1),
                match_id: None,
            });
        }

        assert_eq!(source.event_phase(false), None);
    }

    #[test]
    fn retains_match_id_across_follow_up_phase_events() {
        let source = LocalClientSource::with_lockfile_path(Some(PathBuf::from("unused")));
        source.set_event_phase(GamePhase::InMatch, Some("match-1".into()));
        source.set_event_phase(GamePhase::PostMatch, None);

        assert_eq!(source.event_match_id().as_deref(), Some("match-1"));
    }

    #[test]
    fn parses_valorant_session_shard_and_version() {
        let info = valorant_session_info(&serde_json::json!({
            "session": {
                "productId": "valorant",
                "version": "1.2.3",
                "launchConfiguration": {"arguments": ["-ares-deployment=la2", "-ares-puuid=player-1"]}
            }
        }), None)
        .unwrap();

        assert_eq!(info.shard, "na");
        assert_eq!(info.region, "latam");
        assert_eq!(info.client_version, "1.2.3");
        assert_eq!(info.own_puuid, "player-1");
    }

    #[test]
    fn accepts_puuid_as_a_separate_session_argument() {
        let info = valorant_session_info(&serde_json::json!({
            "session": {
                "productId": "valorant",
                "version": "1.2.3",
                "launchConfiguration": {"arguments": ["-ares-deployment=la2", "-ares-puuid", "player-1"]}
            }
        }), None)
        .unwrap();

        assert_eq!(info.own_puuid, "player-1");
    }

    #[test]
    fn falls_back_to_puuid_from_access_token_subject() {
        let token_payload = URL_SAFE_NO_PAD.encode(r#"{"sub":"player-1"}"#);
        let access_token = format!("header.{token_payload}.signature");
        let info = valorant_session_info(
            &serde_json::json!({
                "session": {
                    "productId": "valorant",
                    "version": "1.2.3",
                    "launchConfiguration": {"arguments": ["-ares-deployment=la2"]}
                }
            }),
            puuid_from_access_token(&access_token).as_deref(),
        )
        .unwrap();

        assert_eq!(info.own_puuid, "player-1");
    }

    #[test]
    fn rejects_malformed_access_token_without_exposing_it() {
        assert_eq!(puuid_from_access_token("not-a-jwt"), None);
    }

    #[test]
    fn in_match_phase_survives_quiet_websocket_while_game_process_is_running() {
        let source = LocalClientSource::with_lockfile_path(Some(PathBuf::from("unused")));
        if let Ok(mut phase) = source.event_phase.lock() {
            *phase = Some(EventPhase {
                phase: GamePhase::InMatch,
                observed_at: Instant::now() - EVENT_PHASE_TTL - Duration::from_secs(1),
                match_id: Some("match-1".into()),
            });
        }

        assert_eq!(source.event_phase(true), Some(GamePhase::InMatch));
        assert_eq!(source.event_phase(false), None);
    }

    #[test]
    fn in_match_phase_expires_even_if_game_process_remains_open() {
        let source = LocalClientSource::with_lockfile_path(Some(PathBuf::from("unused")));
        if let Ok(mut phase) = source.event_phase.lock() {
            *phase = Some(EventPhase {
                phase: GamePhase::InMatch,
                observed_at: Instant::now() - IN_MATCH_PHASE_TTL - Duration::from_secs(1),
                match_id: None,
            });
        }

        assert_eq!(source.event_phase(true), None);
    }
}
