//! Contexto mínimo y de solo lectura de la partida en curso.

use std::time::Duration;

use reqwest::{StatusCode, blocking::Client};
use serde_json::Value;

use super::ProviderError;

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

pub(crate) struct LiveMatchSource(Client);

impl LiveMatchSource {
    pub(crate) fn new() -> Self {
        Self(
            Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("cliente live"),
        )
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
            .0
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
                .map_err(|_| ProviderError::Parse("JSON inválido en partida actual".into()))
                .and_then(|payload| parse_live_match(&payload, &request.own_puuid)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(ProviderError::Unauthorized("GLZ rechazó la sesión".into()))
            }
            status => Err(ProviderError::Unavailable(format!(
                "GLZ respondió HTTP {status}"
            ))),
        }
    }
}

/// Solo datos propios o de la partida; nunca expone el roster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LiveMatchContext {
    pub mode: String,
    pub map: String,
    pub agent: Option<String>,
}

pub(crate) fn parse_live_match(
    payload: &Value,
    own_puuid: &str,
) -> Result<LiveMatchContext, ProviderError> {
    let object = payload
        .as_object()
        .ok_or_else(|| ProviderError::Parse("partida actual inválida".into()))?;
    let mode = required_asset(object, "ModeID")?;
    let map = required_asset(object, "MapID")?;
    let agent = object
        .get("Players")
        .and_then(Value::as_array)
        .and_then(|players| {
            players
                .iter()
                .find(|player| player.get("Subject").and_then(Value::as_str) == Some(own_puuid))
        })
        .and_then(|player| player.get("CharacterID"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(agent_label);
    Ok(LiveMatchContext { mode, map, agent })
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
    fn resolves_internal_map_names_to_public_labels() {
        assert_eq!(asset_label("/Game/Maps/Juliett/Juliett"), "Sunset");
        assert_eq!(asset_label("/Game/Maps/Triad/Triad"), "Haven");
        assert_eq!(asset_label("/Game/Maps/Plummet/Plummet"), "Summit");
    }
}
