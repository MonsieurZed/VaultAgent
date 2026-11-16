//! Dates du front : toujours en UTC, rendues côté serveur (la bibliothèque standard ne connaît aucun fuseau).

const MS_PER_MINUTE: u64 = 60_000;
const MINUTES_PER_DAY: u64 = 1_440;

/// Heure `HH:MM UTC` d'une date ISO 8601 (`2026-11-08T00:15:00.000Z`).
///
/// Paramètres : la date ISO.
/// Retour : l'heure formatée ; un texte qui n'a pas cette forme est rendu tel quel.
#[must_use]
pub fn time_of(iso: &str) -> String {
    clock_part(iso).map_or_else(|| iso.to_owned(), |time| format!("{time} UTC"))
}

/// Date et heure `AAAA-MM-JJ HH:MM UTC` d'une date ISO 8601.
///
/// Paramètres : la date ISO.
/// Retour : la date formatée ; un texte qui n'a pas cette forme est rendu tel quel.
#[must_use]
pub fn date_time_of(iso: &str) -> String {
    match (iso.get(..10), clock_part(iso)) {
        (Some(day), Some(time)) if day.as_bytes().get(4) == Some(&b'-') => format!("{day} {time} UTC"),
        _ => iso.to_owned(),
    }
}

/// Heure `HH:MM UTC` d'un instant en millisecondes depuis l'époque Unix.
///
/// Paramètres : les millisecondes.
/// Retour : l'heure formatée.
#[must_use]
pub fn time_of_ms(ms: u64) -> String {
    let minute_of_day = ms / MS_PER_MINUTE % MINUTES_PER_DAY;
    format!("{:02}:{:02} UTC", minute_of_day / 60, minute_of_day % 60)
}

/// Les cinq caractères `HH:MM` d'une date ISO, si la forme est bonne.
fn clock_part(iso: &str) -> Option<&str> {
    let time = iso.split_once('T')?.1.get(..5)?;
    let bytes = time.as_bytes();
    let digits = |pair: &[u8]| pair.iter().all(u8::is_ascii_digit);
    (bytes.get(2) == Some(&b':') && digits(&bytes[..2]) && digits(&bytes[3..])).then_some(time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_are_cut_to_utc_text() {
        assert_eq!(time_of("2026-11-08T00:15:00.000Z"), "00:15 UTC");
        assert_eq!(date_time_of("2026-11-08T13:05:59.000Z"), "2026-11-08 13:05 UTC");
    }

    #[test]
    fn a_malformed_date_is_kept_as_is() {
        assert_eq!(time_of("demain"), "demain");
        assert_eq!(date_time_of("2026-11-08Tab:cd"), "2026-11-08Tab:cd");
        assert_eq!(date_time_of("é"), "é");
    }

    #[test]
    fn epoch_milliseconds_give_the_time_of_day() {
        assert_eq!(time_of_ms(1_800_000_000_000), "08:00 UTC");
        assert_eq!(time_of_ms(0), "00:00 UTC");
    }
}
