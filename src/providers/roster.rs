//! Normalización auxiliar del roster en vivo.
//!
//! Los PUUID solo se usan como claves efímeras mientras se combina la respuesta
//! de Name Service con Current Game Match. Nunca forman parte del modelo que
//! llega a la interfaz.

use std::collections::HashMap;

use serde_json::Value;

pub(crate) fn visible_names(payload: &Value) -> HashMap<String, String> {
    payload
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let subject = entry.get("Subject").and_then(Value::as_str)?;
            if subject.is_empty() {
                return None;
            }
            // GameName + TagLine es el Riot ID canónico necesario para abrir
            // Tracker.gg. DisplayName puede llegar sin `#tag` en algunas fases.
            let display = (|| {
                let game_name = entry.get("GameName").and_then(Value::as_str)?;
                let tag_line = entry.get("TagLine").and_then(Value::as_str)?;
                (!game_name.is_empty() && !tag_line.is_empty())
                    .then(|| format!("{game_name}#{tag_line}"))
            })()
            .or_else(|| {
                entry
                    .get("DisplayName")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })?;
            Some((subject.to_owned(), display))
        })
        .collect()
}

pub(crate) fn competitive_tier_label(tier: u64) -> Option<String> {
    let (rank, division) = match tier {
        3..=5 => ("Hierro", tier - 2),
        6..=8 => ("Bronce", tier - 5),
        9..=11 => ("Plata", tier - 8),
        12..=14 => ("Oro", tier - 11),
        15..=17 => ("Platino", tier - 14),
        18..=20 => ("Diamante", tier - 17),
        21..=23 => ("Ascendente", tier - 20),
        24..=26 => ("Inmortal", tier - 23),
        27 => return Some("Radiante".into()),
        _ => return None,
    };
    Some(format!("{rank} {division}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_name_service_formats_without_other_fields() {
        let names = visible_names(&serde_json::json!([
            {"Subject":"one", "DisplayName":"Visible#LAN"},
            {"Subject":"two", "GameName":"Segundo", "TagLine":"LAS", "Extra":"ignored"},
            {"Subject":"", "DisplayName":"invalid"}
        ]));
        assert_eq!(names.get("one").map(String::as_str), Some("Visible#LAN"));
        assert_eq!(names.get("two").map(String::as_str), Some("Segundo#LAS"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn prefers_canonical_riot_id_when_display_name_has_no_tag() {
        let names = visible_names(&serde_json::json!([{
            "Subject":"one",
            "DisplayName":"Visible",
            "GameName":"Visible",
            "TagLine":"LAS"
        }]));

        assert_eq!(names.get("one").map(String::as_str), Some("Visible#LAS"));
    }

    #[test]
    fn maps_competitive_tiers_and_leaves_unknown_values_empty() {
        assert_eq!(competitive_tier_label(3).as_deref(), Some("Hierro 1"));
        assert_eq!(competitive_tier_label(18).as_deref(), Some("Diamante 1"));
        assert_eq!(competitive_tier_label(27).as_deref(), Some("Radiante"));
        assert_eq!(competitive_tier_label(0), None);
        assert_eq!(competitive_tier_label(99), None);
    }
}
