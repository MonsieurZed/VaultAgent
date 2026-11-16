//! Horloge du moteur : date ISO 8601 UTC, sans bibliothèque de dates, pour les champs que le SDK exige datés.

use std::time::{SystemTime, UNIX_EPOCH};

/// Date ISO 8601 UTC d'un instant, en secondes.
///
/// Paramètres : les secondes écoulées depuis 1970.
/// Retour : `AAAA-MM-JJTHH:MM:SSZ`.
pub(super) fn iso_from_epoch(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Date ISO courante.
///
/// Retour : l'instant présent, ou l'époque si l'horloge est antérieure à 1970.
pub fn iso_now() -> String {
    iso_from_epoch(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_and_known_dates_are_formatted() {
        assert_eq!(iso_from_epoch(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_epoch(1_784_199_845), "2026-07-16T11:04:05Z");
        assert_eq!(iso_from_epoch(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn now_has_the_iso_shape() {
        let now = iso_now();
        assert_eq!((now.len(), now.ends_with('Z')), (20, true));
    }
}
