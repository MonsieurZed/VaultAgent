//! Lecture des dates ISO 8601 du serveur (`expiresUtc`) en secondes Unix, sans dépendance de calendrier.

/// Jours écoulés depuis 1970-01-01 pour une date du calendrier grégorien.
///
/// Paramètres : année, mois (1 à 12) et jour.
/// Retour : le nombre de jours, négatif avant 1970.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted = (month + 9) % 12;
    let day_of_year = (153 * shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Lit un nombre de chiffres décimaux à une position.
///
/// Paramètres : le texte, le début et la longueur.
/// Retour : la valeur, ou `None` si ce ne sont pas tous des chiffres.
fn digits(text: &str, start: usize, length: usize) -> Option<i64> {
    let part = text.get(start..start + length)?;
    part.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| part.parse().ok())?
}

/// Décalage horaire en secondes d'un suffixe `Z`, `+hh:mm` ou `-hh:mm`.
///
/// Paramètres : le suffixe (après les secondes et leur fraction).
/// Retour : le décalage, ou `None` si le suffixe est invalide.
fn offset_seconds(suffix: &str) -> Option<i64> {
    if suffix == "Z" {
        return Some(0);
    }
    let sign = match suffix.get(..1)? {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let hours = digits(suffix, 1, 2)?;
    let minutes = digits(suffix, 4, 2)?;
    (suffix.len() == 6 && suffix.get(3..4) == Some(":")).then_some(sign * (hours * 3600 + minutes * 60))
}

/// Convertit une date ISO 8601 en secondes Unix.
///
/// Paramètres : un texte `AAAA-MM-JJThh:mm:ss[.fraction](Z|±hh:mm)`.
/// Retour : les secondes Unix (fraction ignorée), ou `None` si le texte est invalide.
#[must_use]
pub fn parse_unix_seconds(text: &str) -> Option<i64> {
    let (year, month, day) = (digits(text, 0, 4)?, digits(text, 5, 2)?, digits(text, 8, 2)?);
    let (hour, minute, second) = (digits(text, 11, 2)?, digits(text, 14, 2)?, digits(text, 17, 2)?);
    let separators = [(4, "-"), (7, "-"), (10, "T"), (13, ":"), (16, ":")];
    if separators.iter().any(|(at, sep)| text.get(*at..*at + 1) != Some(sep)) {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = text.get(19..)?;
    if let Some(fraction) = rest.strip_prefix('.') {
        let length = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if length == 0 {
            return None;
        }
        rest = &fraction[length..];
    }
    let local = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Some(local - offset_seconds(rest)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates_convert() {
        assert_eq!(parse_unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_unix_seconds("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(parse_unix_seconds("2026-11-16T10:00:00.123Z"), Some(1_794_823_200));
    }

    #[test]
    fn offsets_apply() {
        assert_eq!(parse_unix_seconds("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(parse_unix_seconds("1969-12-31T19:00:00-05:00"), Some(0));
    }

    #[test]
    fn invalid_texts_are_refused() {
        for text in [
            "",
            "2026-11-16",
            "2026-11-16T10:00:00",
            "2026-13-16T10:00:00Z",
            "2026-11-16 10:00:00Z",
        ] {
            assert_eq!(parse_unix_seconds(text), None, "{text}");
        }
        assert_eq!(parse_unix_seconds("2026-11-16T10:00:00.Z"), None);
        assert_eq!(parse_unix_seconds("2026-11-16T10:00:00+0100"), None);
    }
}
