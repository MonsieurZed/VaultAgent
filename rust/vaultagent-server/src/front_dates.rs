//! Dates du front : rendues côté serveur en heure de Paris (voir `front_paris`), sans mention de fuseau.

use crate::front_paris::{Local, days_from_civil, local};

const MS_PER_SECOND: i64 = 1_000;

/// Nombre lu dans une tranche de chiffres de la date ISO.
fn number(iso: &str, from: usize, to: usize) -> Option<i64> {
    let digits = iso.get(from..to)?;
    if digits.bytes().all(|byte| byte.is_ascii_digit()) {
        digits.parse().ok()
    } else {
        None
    }
}

/// Instant d'une date ISO 8601 (`2026-11-08T00:15:00.000Z`).
fn iso_seconds(iso: &str) -> Option<i64> {
    let bytes = iso.as_bytes();
    let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':')];
    if !separators
        .iter()
        .all(|(index, expected)| bytes.get(*index) == Some(expected))
    {
        return None;
    }
    let days = days_from_civil(number(iso, 0, 4)?, number(iso, 5, 7)?, number(iso, 8, 10)?);
    Some(days * 86_400 + number(iso, 11, 13)? * 3_600 + number(iso, 14, 16)? * 60)
}

/// Heure `HH:MM` locale.
fn clock(at: &Local) -> String {
    format!("{:02}:{:02}", at.hour, at.minute)
}

/// Heure de Paris d'une date ISO 8601.
///
/// Paramètres : la date ISO.
/// Retour : `HH:MM` ; un texte qui n'a pas cette forme est rendu tel quel.
#[must_use]
pub fn time_of(iso: &str) -> String {
    iso_seconds(iso).map_or_else(|| iso.to_owned(), |seconds| clock(&local(seconds)))
}

/// Jour de Paris d'une date ISO 8601.
///
/// Paramètres : la date ISO.
/// Retour : `JJ/MM` ; un texte qui n'a pas cette forme est rendu tel quel.
#[must_use]
pub fn date_of(iso: &str) -> String {
    iso_seconds(iso).map_or_else(
        || iso.to_owned(),
        |seconds| {
            let at = local(seconds);
            format!("{:02}/{:02}", at.day, at.month)
        },
    )
}

/// Heure de Paris d'un instant en millisecondes depuis l'époque Unix.
///
/// Paramètres : les millisecondes.
/// Retour : `HH:MM`.
#[must_use]
pub fn time_of_ms(ms: u64) -> String {
    clock(&local(i64::try_from(ms).unwrap_or(i64::MAX) / MS_PER_SECOND))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_are_rendered_in_paris_time() {
        assert_eq!(time_of("2026-11-08T00:15:00.000Z"), "01:15");
        assert_eq!(time_of("2026-07-08T00:15:00.000Z"), "02:15");
        assert_eq!(date_of("2026-10-30T23:30:00.000Z"), "31/10");
        assert_eq!(date_of("2026-12-31T23:30:00.000Z"), "01/01");
    }

    #[test]
    fn a_malformed_date_is_kept_as_is() {
        for bad in ["demain", "2026-11-08Tab:cd", "é", "2026/11/08T10:00", ""] {
            assert_eq!(time_of(bad), bad);
            assert_eq!(date_of(bad), bad);
        }
    }

    #[test]
    fn epoch_milliseconds_give_the_paris_time_of_day() {
        assert_eq!(time_of_ms(1_800_000_000_000), "09:00");
        assert_eq!(time_of_ms(0), "01:00");
    }
}
