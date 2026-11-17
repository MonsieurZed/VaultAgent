//! Heure de Paris sans base de fuseaux : UTC+1, et UTC+2 du dernier dimanche de mars 01:00 UTC au dernier dimanche
//! d'octobre 01:00 UTC.

const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;

/// Date et heure locales de Paris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    /// Mois (1 à 12).
    pub month: i64,
    /// Jour du mois (1 à 31).
    pub day: i64,
    /// Heure (0 à 23).
    pub hour: i64,
    /// Minute (0 à 59).
    pub minute: i64,
}

/// Nombre de jours depuis 1970-01-01 d'une date du calendrier grégorien.
///
/// Paramètres : l'année, le mois (1 à 12), le jour.
/// Retour : le numéro de jour (négatif avant 1970).
#[must_use]
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Date du calendrier grégorien d'un numéro de jour.
///
/// Paramètres : les jours depuis 1970-01-01.
/// Retour : l'année, le mois et le jour.
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Numéro de jour du dernier dimanche d'un mois.
fn last_sunday(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let last_day = days_from_civil(next_year, next_month, 1) - 1;
    let monday_based = (last_day + 3).rem_euclid(7);
    last_day - (monday_based + 1) % 7
}

/// Décalage de Paris par rapport à UTC à un instant donné.
///
/// Paramètres : les secondes depuis l'époque Unix.
/// Retour : 1 en hiver, 2 en été (bascules à 01:00 UTC).
#[must_use]
pub fn offset_hours(utc_seconds: i64) -> i64 {
    let (year, _, _) = civil_from_days(utc_seconds.div_euclid(SECONDS_PER_DAY));
    let start = last_sunday(year, 3) * SECONDS_PER_DAY + SECONDS_PER_HOUR;
    let end = last_sunday(year, 10) * SECONDS_PER_DAY + SECONDS_PER_HOUR;
    if (start..end).contains(&utc_seconds) { 2 } else { 1 }
}

/// Date et heure de Paris d'un instant.
///
/// Paramètres : les secondes depuis l'époque Unix.
/// Retour : mois, jour, heure et minute locaux.
#[must_use]
pub fn local(utc_seconds: i64) -> Local {
    let shifted = utc_seconds + offset_hours(utc_seconds) * SECONDS_PER_HOUR;
    let (_, month, day) = civil_from_days(shifted.div_euclid(SECONDS_PER_DAY));
    let second_of_day = shifted.rem_euclid(SECONDS_PER_DAY);
    Local {
        month,
        day,
        hour: second_of_day / SECONDS_PER_HOUR,
        minute: second_of_day % SECONDS_PER_HOUR / 60,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Secondes depuis l'époque d'une date et d'une heure UTC.
    ///
    /// Paramètres : année, mois, jour, heure, minute.
    /// Retour : l'instant en secondes.
    fn at(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> i64 {
        days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * SECONDS_PER_HOUR + minute * 60
    }

    #[test]
    fn the_calendar_round_trips() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2028, 2, 29)), (2028, 2, 29));
        assert_eq!(civil_from_days(days_from_civil(2026, 12, 31)), (2026, 12, 31));
    }

    #[test]
    fn the_last_sundays_of_march_and_october_2026_are_found() {
        assert_eq!(civil_from_days(last_sunday(2026, 3)), (2026, 3, 29));
        assert_eq!(civil_from_days(last_sunday(2026, 10)), (2026, 10, 25));
        assert_eq!(civil_from_days(last_sunday(2027, 3)), (2027, 3, 28));
    }

    #[test]
    fn summer_time_starts_and_ends_at_01_00_utc() {
        assert_eq!(offset_hours(at(2026, 3, 29, 0, 59)), 1);
        assert_eq!(offset_hours(at(2026, 3, 29, 1, 0)), 2);
        assert_eq!(offset_hours(at(2026, 10, 25, 0, 59)), 2);
        assert_eq!(offset_hours(at(2026, 10, 25, 1, 0)), 1);
    }

    #[test]
    fn each_season_has_its_offset() {
        assert_eq!(offset_hours(at(2027, 1, 15, 8, 0)), 1);
        assert_eq!(offset_hours(at(2026, 7, 14, 12, 0)), 2);
        assert_eq!(offset_hours(at(2026, 12, 31, 23, 0)), 1);
    }

    #[test]
    fn local_time_crosses_midnight_and_month_ends() {
        let winter = local(at(2026, 10, 31, 23, 30));
        assert_eq!((winter.month, winter.day, winter.hour, winter.minute), (11, 1, 0, 30));
        let winter = local(at(2026, 12, 31, 23, 30));
        assert_eq!((winter.month, winter.day, winter.hour), (1, 1, 0));
        let summer = local(at(2026, 7, 1, 22, 5));
        assert_eq!((summer.month, summer.day, summer.hour, summer.minute), (7, 2, 0, 5));
    }
}
