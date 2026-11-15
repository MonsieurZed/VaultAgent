//! Date de révision ISO 8601 : lecture stricte, comparaison par instant (millisecondes depuis l'époque).

/// Date de révision d'une fiche, texte d'origine conservé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionDate {
    text: String,
    millis: i64,
}

/// Lit exactement `count` chiffres au début de `bytes`.
fn digits(bytes: &[u8], count: usize) -> Option<(i64, &[u8])> {
    let (head, tail) = bytes.split_at_checked(count)?;
    head.iter()
        .all(u8::is_ascii_digit)
        .then(|| (head.iter().fold(0, |acc, d| acc * 10 + i64::from(d - b'0')), tail))
}

/// Nombre de jours écoulés entre le 1er janvier 1970 et la date civile donnée (calendrier grégorien).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Nombre de jours d'un mois.
fn days_in_month(year: i64, month: i64) -> i64 {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Lit une fraction de seconde (au moins un chiffre) et rend les millisecondes, tronquées comme `Date.parse`.
fn fraction(bytes: &[u8]) -> Option<(i64, &[u8])> {
    let Some(rest) = bytes.strip_prefix(b".") else {
        return Some((0, bytes));
    };
    let count = rest.iter().take_while(|b| b.is_ascii_digit()).count();
    (count > 0).then_some(())?;
    let millis = rest[..count.min(3)]
        .iter()
        .fold(0, |acc, d| acc * 10 + i64::from(d - b'0'));
    let scale = 10_i64.pow(u32::try_from(3 - count.min(3)).ok()?);
    Some((millis * scale, &rest[count..]))
}

/// Lit le décalage horaire final (`Z`, `+HH:MM` ou `-HH:MM`) et rend ses minutes ; une date sans décalage est en UTC.
fn offset_minutes(bytes: &[u8]) -> Option<i64> {
    let sign = match bytes.first() {
        None | Some(b'Z') if bytes.len() <= 1 => return Some(0),
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return None,
    };
    let (hours, rest) = digits(&bytes[1..], 2)?;
    let (minutes, rest) = digits(rest.strip_prefix(b":")?, 2)?;
    (rest.is_empty() && hours < 24 && minutes < 60).then_some(sign * (hours * 60 + minutes))
}

impl RevisionDate {
    /// Lit une date `AAAA-MM-JJ`, éventuellement suivie de `THH:MM[:SS[.fff]]` et d'un décalage.
    ///
    /// Paramètres : le texte candidat. Une date sans décalage est lue en UTC.
    /// Retour : la date, ou `None` si le texte n'est pas une date ISO valide.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let (year, rest) = digits(bytes, 4)?;
        let (month, rest) = digits(rest.strip_prefix(b"-")?, 2)?;
        let (day, rest) = digits(rest.strip_prefix(b"-")?, 2)?;
        ((1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day)).then_some(())?;
        let (clock, offset) = match rest.strip_prefix(b"T") {
            None => (0, if rest.is_empty() { 0 } else { return None }),
            Some(time) => Self::clock(time)?,
        };
        let millis = (days_from_civil(year, month, day) * 86_400_000) + clock - offset * 60_000;
        Some(Self {
            text: text.to_owned(),
            millis,
        })
    }

    /// Lit `HH:MM[:SS[.fff]]` suivi du décalage ; rend les millisecondes de la journée et les minutes de décalage.
    fn clock(time: &[u8]) -> Option<(i64, i64)> {
        let (hour, rest) = digits(time, 2)?;
        let (minute, rest) = digits(rest.strip_prefix(b":")?, 2)?;
        let (second, rest) = match rest.strip_prefix(b":") {
            Some(more) => digits(more, 2)?,
            None => (0, rest),
        };
        let (milli, rest) = fraction(rest)?;
        let in_range = minute < 60 && second < 60 && (hour < 24 || hour == 24 && minute + second + milli == 0);
        in_range.then_some(())?;
        let day_millis = ((hour * 60 + minute) * 60 + second) * 1000 + milli;
        Some((day_millis, offset_minutes(rest)?))
    }

    /// Texte d'origine.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Instant en millisecondes depuis le 1er janvier 1970 UTC.
    #[must_use]
    pub fn millis(&self) -> i64 {
        self.millis
    }

    /// Indique si deux dates désignent le même instant, quelle que soit leur écriture.
    #[must_use]
    pub fn same_instant(&self, other: &Self) -> bool {
        self.millis == other.millis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_instants() {
        assert_eq!(
            RevisionDate::parse("1970-01-01T00:00:00.000Z").map(|d| d.millis()),
            Some(0)
        );
        assert_eq!(
            RevisionDate::parse("2026-11-08T00:00:00.000Z").map(|d| d.millis()),
            Some(1_794_096_000_000)
        );
        assert_eq!(
            RevisionDate::parse("2026-11-08").map(|d| d.millis()),
            Some(1_794_096_000_000)
        );
    }

    #[test]
    fn equal_instants_with_different_writings() {
        let a = RevisionDate::parse("2026-11-08T00:00:00.000Z");
        let b = RevisionDate::parse("2026-11-08T01:00:00+01:00");
        let c = RevisionDate::parse("2026-11-08T00:00:00.0000000Z");
        assert!(a.as_ref().zip(b.as_ref()).is_some_and(|(x, y)| x.same_instant(y)));
        assert!(a.as_ref().zip(c.as_ref()).is_some_and(|(x, y)| x.same_instant(y)));
    }

    #[test]
    fn keeps_original_text_and_truncates_fraction() {
        let date = RevisionDate::parse("2026-11-08T00:00:00.1239Z");
        assert_eq!(
            date.as_ref().map(RevisionDate::as_str),
            Some("2026-11-08T00:00:00.1239Z")
        );
        assert_eq!(date.map(|d| d.millis() % 1000), Some(123));
    }

    #[test]
    fn refuses_invalid_dates() {
        for bad in [
            "",
            "pas une date",
            "2026",
            "2026-13-01",
            "2026-02-30",
            "2025-02-29T00:00:00Z",
            "2026-11-08T25:00:00Z",
            "2026-11-08T10:60:00Z",
            "2026-11-08T10:00:61Z",
            "2026-11-08T10:00:00.Z",
            "2026-11-08T10:00:00+0100",
            "2026-11-08 10:00:00Z",
            "2026-11-08T10:00:00Zx",
            "2026-11-08x",
        ] {
            assert!(RevisionDate::parse(bad).is_none(), "{bad}");
        }
        assert!(RevisionDate::parse("2024-02-29T24:00:00Z").is_some());
        assert!(RevisionDate::parse("2024-02-29T24:00:01Z").is_none());
    }
}
