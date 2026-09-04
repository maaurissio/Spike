//! Enriquecimiento histórico efímero del roster en curso.
//!
//! Los PUUID y MatchID viven únicamente durante `fetch`: se usan para unir
//! historial y detalles, se deduplican y se descartan antes de volver a la TUI.

use std::{
    collections::{HashMap, HashSet},
    thread,
    time::Duration,
};

use reqwest::{StatusCode, blocking::Client};
use serde_json::Value;

use crate::models::{MatchOutcome, roster::HistoricalStats};

use super::ProviderError;

const CLIENT_PLATFORM: &str = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const HISTORY_MATCHES: usize = 5;
const MAX_PLAYERS: usize = 12;
const MAX_CONCURRENCY: usize = 6;
// Tracker Network no publica su ventana exacta. Spike usa una definición
// explícita y estable: la baja debe ser respondida por un compañero en 5 s.
const TRADE_WINDOW_MS: u64 = 5_000;

#[derive(Clone)]
pub(crate) struct RosterStatsRequest {
    pub shard: String,
    pub client_version: String,
    pub access_token: String,
    pub entitlement_token: String,
}

impl std::fmt::Debug for RosterStatsRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RosterStatsRequest")
            .field("shard", &self.shard)
            .field("client_version", &self.client_version)
            .field("access_token", &"<redacted>")
            .field("entitlement_token", &"<redacted>")
            .finish()
    }
}

pub(crate) struct RosterStatsSource {
    client: Client,
    base_url: Option<String>,
}

pub(crate) struct RosterEnrichment {
    pub stats: HashMap<String, HistoricalStats>,
    pub peaks: HashMap<String, u64>,
    /// PUUID -> identificador opaco de grupo inferido. No contiene MatchID.
    pub inferred_parties: HashMap<String, String>,
}

impl RosterStatsSource {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de estadísticas del roster debe poder construirse"),
            base_url: None,
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("el cliente de estadísticas de prueba debe poder construirse"),
            base_url: Some(base_url),
        }
    }

    /// Obtiene como máximo cinco partidas por jugador. Los historiales y los
    /// detalles se consultan con concurrencia limitada; una falla afecta solo
    /// al jugador o partida correspondiente.
    pub(crate) fn fetch(
        &self,
        request: &RosterStatsRequest,
        subjects: &[String],
        excluded_match_id: Option<&str>,
    ) -> RosterEnrichment {
        let subjects = unique_subjects(subjects);
        let mut histories = HashMap::<String, Vec<String>>::new();
        let mut peaks = HashMap::new();

        for chunk in subjects.chunks(MAX_CONCURRENCY) {
            let fetched = thread::scope(|scope| {
                let handles = chunk
                    .iter()
                    .cloned()
                    .map(|subject| {
                        scope.spawn(move || {
                            let history = self.fetch_history(request, &subject);
                            let peak = self.fetch_peak(request, &subject).ok().flatten();
                            (subject, history, peak)
                        })
                    })
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .filter_map(|handle| handle.join().ok())
                    .collect::<Vec<_>>()
            });
            for (subject, history, peak) in fetched {
                if let Some(peak) = peak {
                    peaks.insert(subject.clone(), peak);
                }
                if let Ok(mut match_ids) = history {
                    exclude_match(&mut match_ids, excluded_match_id);
                    histories.insert(subject, match_ids);
                }
            }
        }

        let match_ids = unique_match_ids(&histories);
        let mut details = HashMap::<String, Value>::new();
        for chunk in match_ids.chunks(MAX_CONCURRENCY) {
            let fetched = thread::scope(|scope| {
                let handles = chunk
                    .iter()
                    .cloned()
                    .map(|match_id| {
                        scope.spawn(move || {
                            let detail = self.fetch_detail(request, &match_id);
                            (match_id, detail)
                        })
                    })
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .filter_map(|handle| handle.join().ok())
                    .collect::<Vec<_>>()
            });
            for (match_id, detail) in fetched {
                if let Ok(payload) = detail {
                    details.insert(match_id, payload);
                }
            }
        }

        let inferred_parties = inferred_parties(&histories, &details);

        let stats = histories
            .into_iter()
            .filter_map(|(subject, match_ids)| {
                let mut stats = HistoricalStats::default();
                for match_id in match_ids {
                    if let Some(payload) = details.get(&match_id) {
                        aggregate_match(payload, &subject, &mut stats);
                    }
                }
                (stats.matches > 0).then_some((subject, stats))
            })
            .collect();
        RosterEnrichment {
            stats,
            peaks,
            inferred_parties,
        }
    }

    fn fetch_peak(
        &self,
        request: &RosterStatsRequest,
        subject: &str,
    ) -> Result<Option<u64>, ProviderError> {
        if !safe_identifier(subject) {
            return Err(ProviderError::Parse(
                "identificador de jugador inválido en MMR".into(),
            ));
        }
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/mmr/v1/players/{subject}");
        let response = authenticated(self.client.get(url), request)
            .send()
            .map_err(|_| ProviderError::Network("no se pudo consultar MMR de roster".into()))?;
        let payload = response_json(response, "MMR del roster")?;
        Ok(peak_tier(&payload))
    }

    fn fetch_history(
        &self,
        request: &RosterStatsRequest,
        subject: &str,
    ) -> Result<Vec<String>, ProviderError> {
        if !safe_identifier(subject) {
            return Err(ProviderError::Parse(
                "identificador de jugador inválido en roster".into(),
            ));
        }
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/match-history/v1/history/{subject}");
        let builder = self.client.get(url).query(&[
            ("startIndex", "0"),
            ("endIndex", "5"),
            ("queue", "competitive"),
        ]);
        let response = authenticated(builder, request).send().map_err(|_| {
            ProviderError::Network("no se pudo consultar historial de roster".into())
        })?;
        let payload = response_json(response, "match-history del roster")?;
        parse_history(&payload, subject)
    }

    fn fetch_detail(
        &self,
        request: &RosterStatsRequest,
        match_id: &str,
    ) -> Result<Value, ProviderError> {
        if !safe_identifier(match_id) {
            return Err(ProviderError::Parse(
                "identificador de partida inválido en historial".into(),
            ));
        }
        let base = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("https://pd.{}.a.pvp.net", request.shard));
        let url = format!("{base}/match-details/v1/matches/{match_id}");
        let response = authenticated(self.client.get(url), request)
            .send()
            .map_err(|_| ProviderError::Network("no se pudo consultar detalle de roster".into()))?;
        response_json(response, "match-details del roster")
    }
}

fn peak_tier(payload: &Value) -> Option<u64> {
    payload
        .pointer("/QueueSkills/competitive/SeasonalInfoBySeasonID")?
        .as_object()?
        .values()
        .filter_map(|season| {
            let tier = season.get("CompetitiveTier")?.as_u64()?;
            (tier >= 3).then_some((
                tier,
                season
                    .get("RankedRating")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            ))
        })
        .max()
        .map(|(tier, _)| tier)
}

/// Durante una partida Riot no publica de forma fiable el PartyID enemigo.
/// El último encuentro terminado sí contiene el PartyID autoritativo: solo se
/// conserva un grupo si al menos dos integrantes del roster compartían ese
/// PartyID. Coincidir meramente en una partida anterior no basta.
fn inferred_parties(
    histories: &HashMap<String, Vec<String>>,
    details: &HashMap<String, Value>,
) -> HashMap<String, String> {
    let mut by_latest = HashMap::<(&str, &str), Vec<&str>>::new();
    for (subject, matches) in histories {
        let Some(latest) = matches.first() else {
            continue;
        };
        let Some(party) = details
            .get(latest)
            .and_then(|payload| completed_party_id(payload, subject))
        else {
            continue;
        };
        by_latest.entry((latest, party)).or_default().push(subject);
    }
    let mut groups = by_latest
        .into_values()
        .filter(|members| members.len() > 1)
        .collect::<Vec<_>>();
    groups.sort_by_key(|members| members.iter().min().copied().unwrap_or_default());
    let mut result = HashMap::new();
    for (index, members) in groups.into_iter().enumerate() {
        let group = format!("inferred-{}", index + 1);
        for subject in members {
            result.insert(subject.to_owned(), group.clone());
        }
    }
    result
}

fn completed_party_id<'a>(payload: &'a Value, subject: &str) -> Option<&'a str> {
    let player = payload
        .get("players")
        .and_then(Value::as_array)?
        .iter()
        .find(|player| player.get("subject").and_then(Value::as_str) == Some(subject))?;
    ["partyId", "partyID", "PartyID"]
        .iter()
        .find_map(|key| player.get(*key).and_then(Value::as_str))
        .filter(|party| safe_identifier(party))
}

fn exclude_match(matches: &mut Vec<String>, excluded_match_id: Option<&str>) {
    if let Some(excluded) = excluded_match_id {
        matches.retain(|match_id| !match_id.eq_ignore_ascii_case(excluded));
    }
}

impl Default for RosterStatsSource {
    fn default() -> Self {
        Self::new()
    }
}

fn authenticated(
    builder: reqwest::blocking::RequestBuilder,
    request: &RosterStatsRequest,
) -> reqwest::blocking::RequestBuilder {
    builder
        .header("X-Riot-ClientPlatform", CLIENT_PLATFORM)
        .header("X-Riot-ClientVersion", &request.client_version)
        .header("X-Riot-Entitlements-JWT", &request.entitlement_token)
        .bearer_auth(&request.access_token)
}

fn response_json(
    response: reqwest::blocking::Response,
    endpoint: &str,
) -> Result<Value, ProviderError> {
    match response.status() {
        status if status.is_success() => response
            .json::<Value>()
            .map_err(|_| ProviderError::Parse(format!("JSON inválido en {endpoint}"))),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Unauthorized(
            format!("PD rechazó la sesión en {endpoint}"),
        )),
        StatusCode::TOO_MANY_REQUESTS => Err(ProviderError::RateLimited(format!(
            "PD limitó temporalmente {endpoint}"
        ))),
        status => Err(ProviderError::Unavailable(format!(
            "PD respondió HTTP {status} en {endpoint}"
        ))),
    }
}

fn unique_subjects(subjects: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    subjects
        .iter()
        .filter(|subject| !subject.is_empty())
        .filter(|subject| seen.insert((*subject).clone()))
        .take(MAX_PLAYERS)
        .cloned()
        .collect()
}

fn unique_match_ids(histories: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut seen = HashSet::new();
    histories
        .values()
        .flat_map(|matches| matches.iter())
        .filter(|match_id| seen.insert((*match_id).clone()))
        .cloned()
        .collect()
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn parse_history(payload: &Value, subject: &str) -> Result<Vec<String>, ProviderError> {
    if payload.get("Subject").and_then(Value::as_str) != Some(subject) {
        return Err(ProviderError::Parse(
            "match-history no corresponde al jugador solicitado".into(),
        ));
    }
    let history = payload
        .get("History")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Parse("History ausente en roster".into()))?;
    Ok(history
        .iter()
        .filter_map(|entry| entry.get("MatchID").and_then(Value::as_str))
        .filter(|match_id| safe_identifier(match_id))
        .take(HISTORY_MATCHES)
        .map(ToOwned::to_owned)
        .collect())
}

fn aggregate_match(payload: &Value, subject: &str, total: &mut HistoricalStats) -> bool {
    let Some(players) = payload.get("players").and_then(Value::as_array) else {
        return false;
    };
    let Some(player) = players
        .iter()
        .find(|player| player.get("subject").and_then(Value::as_str) == Some(subject))
    else {
        return false;
    };
    let Some(stats) = player.get("stats").and_then(Value::as_object) else {
        return false;
    };
    let (Some(kills), Some(deaths), Some(assists)) = (
        value_u32(stats.get("kills")),
        value_u32(stats.get("deaths")),
        value_u32(stats.get("assists")),
    ) else {
        return false;
    };

    total.matches = total.matches.saturating_add(1);
    if total.competitive_tier.is_none() {
        total.competitive_tier = player
            .get("competitiveTier")
            .and_then(Value::as_u64)
            .filter(|tier| *tier > 0);
    }
    if total.account_level.is_none() {
        total.account_level = value_u32(player.get("accountLevel")).filter(|level| *level > 0);
    }
    total.kills = total.kills.saturating_add(kills);
    total.deaths = total.deaths.saturating_add(deaths);
    total.assists = total.assists.saturating_add(assists);

    let outcome = match_outcome(payload, player.get("teamId").and_then(Value::as_str));
    total.recent.push(outcome);
    match outcome {
        MatchOutcome::Win => {
            total.decided_matches = total.decided_matches.saturating_add(1);
            total.wins = total.wins.saturating_add(1);
        }
        MatchOutcome::Loss => {
            total.decided_matches = total.decided_matches.saturating_add(1);
        }
        MatchOutcome::Draw | MatchOutcome::Unknown => {}
    }

    if let Some(rounds) = payload.get("roundResults").and_then(Value::as_array) {
        let teams = player_teams(players);
        for round in rounds {
            if let Some(player_stats) = round_player_stats(round, subject) {
                add_shots(player_stats, total);
            }
            if let Some(kast) = round_kast(round, subject, &teams) {
                total.rounds_played = total.rounds_played.saturating_add(1);
                if kast {
                    total.kast_rounds = total.kast_rounds.saturating_add(1);
                }
            }
        }
    }
    true
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value?.as_u64().and_then(|value| u32::try_from(value).ok())
}

fn match_outcome(payload: &Value, team_id: Option<&str>) -> MatchOutcome {
    if payload
        .pointer("/matchInfo/completionState")
        .and_then(Value::as_str)
        == Some("VoteDraw")
    {
        return MatchOutcome::Draw;
    }
    let Some(team_id) = team_id else {
        return MatchOutcome::Unknown;
    };
    payload
        .get("teams")
        .and_then(Value::as_array)
        .and_then(|teams| {
            teams
                .iter()
                .find(|team| team.get("teamId").and_then(Value::as_str) == Some(team_id))
        })
        .and_then(|team| team.get("won").and_then(Value::as_bool))
        .map_or(MatchOutcome::Unknown, |won| {
            if won {
                MatchOutcome::Win
            } else {
                MatchOutcome::Loss
            }
        })
}

fn player_teams(players: &[Value]) -> HashMap<&str, &str> {
    players
        .iter()
        .filter_map(|player| {
            Some((
                player.get("subject")?.as_str()?,
                player.get("teamId")?.as_str()?,
            ))
        })
        .collect()
}

fn round_player_stats<'a>(round: &'a Value, subject: &str) -> Option<&'a Value> {
    round
        .get("playerStats")?
        .as_array()?
        .iter()
        .find(|stats| stats.get("subject").and_then(Value::as_str) == Some(subject))
}

fn add_shots(player_stats: &Value, total: &mut HistoricalStats) {
    let Some(damage) = player_stats.get("damage").and_then(Value::as_array) else {
        return;
    };
    for entry in damage {
        total.headshots = total
            .headshots
            .saturating_add(value_u32(entry.get("headshots")).unwrap_or(0));
        total.bodyshots = total
            .bodyshots
            .saturating_add(value_u32(entry.get("bodyshots")).unwrap_or(0));
        total.legshots = total
            .legshots
            .saturating_add(value_u32(entry.get("legshots")).unwrap_or(0));
    }
}

fn round_kast(round: &Value, subject: &str, teams: &HashMap<&str, &str>) -> Option<bool> {
    round_player_stats(round, subject)?;
    let own_team = teams
        .get(subject)
        .copied()
        .filter(|team| !team.is_empty())?;
    if !teams.values().any(|team| *team != own_team) {
        return None;
    }

    let kills = round
        .get("playerStats")?
        .as_array()?
        .iter()
        .filter_map(|stats| stats.get("kills").and_then(Value::as_array))
        .flatten()
        .collect::<Vec<_>>();
    let has_kill = kills
        .iter()
        .any(|kill| kill.get("killer").and_then(Value::as_str) == Some(subject));
    let has_assist = kills.iter().any(|kill| {
        kill.get("assistants")
            .and_then(Value::as_array)
            .is_some_and(|assistants| assistants.iter().any(|id| id.as_str() == Some(subject)))
    });
    let deaths = kills
        .iter()
        .filter(|kill| kill.get("victim").and_then(Value::as_str) == Some(subject))
        .copied()
        .collect::<Vec<_>>();
    let survived = deaths.is_empty();
    let traded = deaths.iter().any(|death| {
        let Some(killer) = death.get("killer").and_then(Value::as_str) else {
            return false;
        };
        let Some(death_time) = kill_time(death) else {
            return false;
        };
        kills.iter().any(|trade| {
            trade.get("victim").and_then(Value::as_str) == Some(killer)
                && trade
                    .get("killer")
                    .and_then(Value::as_str)
                    .and_then(|trader| teams.get(trader))
                    .is_some_and(|team| *team == own_team)
                && kill_time(trade).is_some_and(|trade_time| {
                    trade_time >= death_time
                        && trade_time.saturating_sub(death_time) <= TRADE_WINDOW_MS
                })
        })
    });
    Some(has_kill || has_assist || survived || traded)
}

fn kill_time(kill: &Value) -> Option<u64> {
    kill.get("roundTime")
        .or_else(|| kill.get("timeSinceRoundStartMillis"))
        .or_else(|| kill.get("gameTime"))
        .and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    use super::*;

    fn fixture(trade_time: u64) -> Value {
        serde_json::json!({
            "matchInfo": {"completionState": "Completed"},
            "players": [
                {"subject":"hidden", "teamId":"Blue", "stats":{"kills":1,"deaths":1,"assists":0}},
                {"subject":"ally", "teamId":"Blue", "stats":{"kills":1,"deaths":0,"assists":0}},
                {"subject":"enemy", "teamId":"Red", "stats":{"kills":1,"deaths":2,"assists":0}}
            ],
            "teams": [
                {"teamId":"Blue", "won":true},
                {"teamId":"Red", "won":false}
            ],
            "roundResults": [
                {"playerStats":[
                    {"subject":"hidden", "kills":[{"killer":"hidden","victim":"enemy","roundTime":1000,"assistants":[]}], "damage":[{"headshots":1,"bodyshots":0,"legshots":0}]},
                    {"subject":"ally", "kills":[], "damage":[]},
                    {"subject":"enemy", "kills":[], "damage":[]}
                ]},
                {"playerStats":[
                    {"subject":"hidden", "kills":[], "damage":[{"headshots":0,"bodyshots":1,"legshots":0}]},
                    {"subject":"ally", "kills":[{"killer":"ally","victim":"enemy","roundTime":trade_time,"assistants":[]}], "damage":[]},
                    {"subject":"enemy", "kills":[{"killer":"enemy","victim":"hidden","roundTime":1000,"assistants":[]}], "damage":[]}
                ]}
            ]
        })
    }

    #[test]
    fn parses_history_for_requested_subject_without_leaking_other_fields() {
        let matches = parse_history(
            &serde_json::json!({
                "Subject":"hidden",
                "History":[
                    {"MatchID":"one", "QueueID":"competitive"},
                    {"MatchID":"two", "QueueID":"competitive"}
                ]
            }),
            "hidden",
        )
        .unwrap();

        assert_eq!(matches, ["one", "two"]);
    }

    #[test]
    fn aggregates_hidden_player_combat_shots_kast_and_outcome() {
        let mut stats = HistoricalStats::default();
        let mut payload = fixture(4_000);
        payload["players"][0]["competitiveTier"] = serde_json::json!(18);
        payload["players"][0]["accountLevel"] = serde_json::json!(321);

        assert!(aggregate_match(&payload, "hidden", &mut stats));

        assert_eq!(stats.matches, 1);
        assert_eq!(stats.competitive_tier, Some(18));
        assert_eq!(stats.account_level, Some(321));
        assert_eq!(stats.decided_matches, 1);
        assert_eq!(stats.wins, 1);
        assert_eq!((stats.kills, stats.deaths, stats.assists), (1, 1, 0));
        assert_eq!(
            (stats.headshots, stats.bodyshots, stats.legshots),
            (1, 1, 0)
        );
        assert_eq!((stats.kast_rounds, stats.rounds_played), (2, 2));
        assert_eq!(stats.recent, [MatchOutcome::Win]);
    }

    #[test]
    fn does_not_count_a_late_revenge_as_a_trade() {
        let mut stats = HistoricalStats::default();

        aggregate_match(&fixture(7_000), "hidden", &mut stats);

        assert_eq!((stats.kast_rounds, stats.rounds_played), (1, 2));
    }

    #[test]
    fn deduplicates_details_shared_by_roster_players() {
        let histories = HashMap::from([
            ("one".into(), vec!["shared".into(), "first".into()]),
            ("two".into(), vec!["shared".into(), "second".into()]),
        ]);

        let unique = unique_match_ids(&histories);

        assert_eq!(unique.len(), 3);
        assert_eq!(unique.iter().filter(|id| *id == "shared").count(), 1);
    }

    #[test]
    fn infers_live_parties_from_authoritative_party_ids_in_latest_match() {
        let histories = HashMap::from([
            ("ally-a".into(), vec!["shared-a".into(), "older-1".into()]),
            ("ally-b".into(), vec!["shared-a".into(), "older-2".into()]),
            ("same-match-solo".into(), vec!["shared-a".into()]),
            ("enemy-a".into(), vec!["shared-b".into()]),
            ("enemy-b".into(), vec!["shared-b".into(), "older-3".into()]),
            ("solo".into(), vec!["unique".into()]),
        ]);
        let details = HashMap::from([
            (
                "shared-a".into(),
                serde_json::json!({"players":[
                    {"subject":"ally-a","partyId":"party-a"},
                    {"subject":"ally-b","partyId":"party-a"},
                    {"subject":"same-match-solo","partyId":"solo-a"}
                ]}),
            ),
            (
                "shared-b".into(),
                serde_json::json!({"players":[
                    {"subject":"enemy-a","partyId":"party-b"},
                    {"subject":"enemy-b","partyId":"party-b"}
                ]}),
            ),
            (
                "unique".into(),
                serde_json::json!({"players":[
                    {"subject":"solo","partyId":"solo-c"}
                ]}),
            ),
        ]);

        let inferred = inferred_parties(&histories, &details);

        assert_eq!(inferred["ally-a"], inferred["ally-b"]);
        assert_eq!(inferred["enemy-a"], inferred["enemy-b"]);
        assert_ne!(inferred["ally-a"], inferred["enemy-a"]);
        assert!(!inferred.contains_key("solo"));
        assert!(!inferred.contains_key("same-match-solo"));
        assert!(inferred.values().all(|group| !group.contains("shared")));
    }

    #[test]
    fn current_live_match_is_excluded_before_party_inference() {
        let mut histories = HashMap::from([
            ("me".into(), vec!["current".into(), "real-party".into()]),
            ("friend".into(), vec!["current".into(), "real-party".into()]),
            ("ally-3".into(), vec!["current".into(), "unique-3".into()]),
            ("ally-4".into(), vec!["current".into(), "unique-4".into()]),
            ("ally-5".into(), vec!["current".into(), "unique-5".into()]),
        ]);
        for matches in histories.values_mut() {
            exclude_match(matches, Some("current"));
        }
        let details = HashMap::from([
            (
                "real-party".into(),
                serde_json::json!({"players":[
                    {"subject":"me","partyId":"duo"},
                    {"subject":"friend","partyId":"duo"}
                ]}),
            ),
            (
                "unique-3".into(),
                serde_json::json!({"players":[{"subject":"ally-3","partyId":"solo-3"}]}),
            ),
            (
                "unique-4".into(),
                serde_json::json!({"players":[{"subject":"ally-4","partyId":"solo-4"}]}),
            ),
            (
                "unique-5".into(),
                serde_json::json!({"players":[{"subject":"ally-5","partyId":"solo-5"}]}),
            ),
        ]);

        let inferred = inferred_parties(&histories, &details);

        assert_eq!(inferred.len(), 2);
        assert_eq!(inferred["me"], inferred["friend"]);
        assert!(!inferred.contains_key("ally-3"));
        assert!(!inferred.contains_key("ally-4"));
        assert!(!inferred.contains_key("ally-5"));
    }

    #[test]
    fn source_enriches_visible_and_hidden_subjects_and_fetches_shared_detail_once() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..5 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let size = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
                assert!(request.contains("authorization: bearer access"));
                let path = request.lines().next().unwrap_or_default();
                let body = if path.contains("/history/hidden") {
                    assert!(path.contains("queue=competitive"));
                    serde_json::json!({
                        "Subject":"hidden",
                        "History":[{"MatchID":"current"},{"MatchID":"shared"}]
                    })
                } else if path.contains("/history/visible") {
                    assert!(path.contains("queue=competitive"));
                    serde_json::json!({
                        "Subject":"visible",
                        "History":[{"MatchID":"current"},{"MatchID":"shared"}]
                    })
                } else if path.contains("/mmr/v1/players/") {
                    serde_json::json!({
                        "QueueSkills":{"competitive":{"SeasonalInfoBySeasonID":{
                            "old":{"CompetitiveTier":18,"RankedRating":80},
                            "peak":{"CompetitiveTier":21,"RankedRating":10}
                        }}}
                    })
                } else {
                    assert!(path.contains("/matches/shared"));
                    assert!(!path.contains("/matches/current"));
                    serde_json::json!({
                        "matchInfo":{"completionState":"Completed"},
                        "players":[
                            {"subject":"hidden","partyId":"party-a","teamId":"Blue","stats":{"kills":4,"deaths":2,"assists":1}},
                            {"subject":"visible","partyId":"party-a","teamId":"Blue","stats":{"kills":2,"deaths":4,"assists":0}}
                        ],
                        "teams":[
                            {"teamId":"Blue","won":true},
                            {"teamId":"Red","won":false}
                        ]
                    })
                }
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        let source = RosterStatsSource::with_base_url(format!("http://{address}"));
        let request = RosterStatsRequest {
            shard: "na".into(),
            client_version: "version".into(),
            access_token: "access".into(),
            entitlement_token: "entitlement".into(),
        };

        let enrichment = source.fetch(
            &request,
            &["visible".into(), "hidden".into()],
            Some("current"),
        );

        assert_eq!(enrichment.stats.len(), 2);
        assert_eq!(enrichment.peaks.len(), 2);
        assert!(enrichment.peaks.values().all(|tier| *tier == 21));
        assert_eq!(enrichment.stats["hidden"].kd_hundredths(), Some(200));
        assert_eq!(enrichment.stats["visible"].win_rate_tenths(), Some(1000));
        assert_eq!(
            enrichment.inferred_parties["hidden"],
            enrichment.inferred_parties["visible"]
        );
        server.join().unwrap();
    }
}
