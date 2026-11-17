//! Identité exigée de la fenêtre cible d'une saisie directe : processus exact et fragment de titre explicite.

use crate::error::ClientError;
use crate::text_fold::fold;

/// Longueur minimale du fragment de titre, en caractères.
const MIN_TITLE_LENGTH: usize = 3;

/// Longueur maximale du nom de processus.
const MAX_PROCESS_LENGTH: usize = 64;

/// Titres de navigateur ou d'onglet vide, déjà repliés, refusés car ils n'identifient aucune page.
const GENERIC_TITLES: [&str; 16] = [
    "new tab",
    "nouvel onglet",
    "google chrome",
    "chrome",
    "microsoft edge",
    "edge",
    "firefox",
    "mozilla firefox",
    "opera",
    "brave",
    "vivaldi",
    "about:blank",
    "untitled",
    "sans titre",
    "nouvelle fenetre",
    "new window",
];

/// Conditions que la fenêtre active doit remplir avant et pendant la frappe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRequirement {
    process: String,
    title: String,
}

/// Indique si un caractère est admis dans un nom de processus.
///
/// Paramètres : le caractère.
/// Retour : vrai pour une lettre ou un chiffre ASCII, `.`, `_` et `-`.
fn is_process_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
}

/// Normalise et valide un nom de processus : espaces et `.exe` final retirés.
///
/// Paramètres : le nom donné.
/// Retour : le nom normalisé, ou `Usage`.
///
/// # Errors
///
/// `Usage` si le nom est vide, de plus de 64 caractères ou contient autre chose que lettres, chiffres, `.`, `_`, `-`.
fn clean_process(process: &str) -> Result<String, ClientError> {
    let trimmed = process.trim();
    let stem_length = trimmed.len().saturating_sub(4);
    let name = match trimmed.get(stem_length..) {
        Some(tail) if tail.eq_ignore_ascii_case(".exe") => &trimmed[..stem_length],
        _ => trimmed,
    };
    if name.is_empty() || name.len() > MAX_PROCESS_LENGTH || !name.chars().all(is_process_character) {
        return Err(ClientError::Usage(
            "--process must be a plain process name, for example chrome.".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

/// Valide un fragment de titre : assez long, sans caractère de contrôle, pas un titre générique.
///
/// Paramètres : le fragment donné.
/// Retour : le fragment sans espaces de bord, ou `Usage`.
///
/// # Errors
///
/// `Usage` si le fragment a moins de 3 caractères, contient un caractère de contrôle ou est contenu dans un titre
/// générique de navigateur (comparaison sans accents ni casse).
fn clean_title(title: &str) -> Result<String, ClientError> {
    let fragment = title.trim();
    if fragment.chars().count() < MIN_TITLE_LENGTH || fragment.chars().any(char::is_control) {
        return Err(ClientError::Usage(format!(
            "--title must contain at least {MIN_TITLE_LENGTH} visible characters of the page or application title."
        )));
    }
    let folded = fold(fragment);
    if GENERIC_TITLES.iter().any(|generic| generic.contains(folded.as_str())) {
        return Err(ClientError::Usage(
            "--title is too generic (browser name or empty tab); give a fragment specific to the target page."
                .to_owned(),
        ));
    }
    Ok(fragment.to_owned())
}

impl TargetRequirement {
    /// Valide et normalise les exigences.
    ///
    /// Paramètres : le nom du processus, avec ou sans `.exe`, et le fragment de titre exigé.
    /// Retour : les exigences normalisées.
    ///
    /// # Errors
    ///
    /// `Usage` pour un nom de processus invalide, un fragment trop court ou trop générique.
    pub fn create(process: &str, title: &str) -> Result<Self, ClientError> {
        Ok(Self {
            process: clean_process(process)?,
            title: clean_title(title)?,
        })
    }

    /// Nom de processus exigé, sans `.exe`.
    #[must_use]
    pub fn process(&self) -> &str {
        &self.process
    }

    /// Fragment de titre exigé.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Indique si une fenêtre observée satisfait les exigences.
    ///
    /// Paramètres : le nom du processus propriétaire et le titre courant de la fenêtre.
    /// Retour : vrai si le processus est identique (sans casse) et si le titre contient le fragment (sans casse).
    #[must_use]
    pub fn matches(&self, process: &str, title: &str) -> bool {
        process.eq_ignore_ascii_case(&self.process) && title.to_lowercase().contains(&self.title.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Indique si le résultat est une erreur d'usage.
    fn usage(result: &Result<TargetRequirement, ClientError>) -> bool {
        matches!(result, Err(ClientError::Usage(_)))
    }

    #[test]
    fn process_is_trimmed_and_loses_exe() {
        for given in ["chrome", " chrome ", "chrome.exe", "CHROME.EXE"] {
            let requirement = TargetRequirement::create(given, "Banque").unwrap();
            assert_eq!(requirement.process().to_lowercase(), "chrome", "{given}");
        }
        assert_eq!(
            TargetRequirement::create("my-app_2.x", "Banque").unwrap().process(),
            "my-app_2.x"
        );
    }

    #[test]
    fn bad_process_names_are_refused() {
        let long = "a".repeat(65);
        for given in [
            "",
            "   ",
            ".exe",
            "a b",
            "C:\\x\\chrome",
            "chr/ome",
            "caf\u{e9}",
            long.as_str(),
        ] {
            assert!(usage(&TargetRequirement::create(given, "Banque")), "{given:?}");
        }
        assert!(TargetRequirement::create(&"a".repeat(64), "Banque").is_ok());
    }

    #[test]
    fn title_needs_three_visible_characters() {
        assert!(usage(&TargetRequirement::create("chrome", "ab")));
        assert!(usage(&TargetRequirement::create("chrome", "  ab  ")));
        assert!(usage(&TargetRequirement::create("chrome", "ab\tc")));
        assert!(usage(&TargetRequirement::create("chrome", "abc\u{7}")));
        assert_eq!(TargetRequirement::create("chrome", "  abc  ").unwrap().title(), "abc");
    }

    #[test]
    fn generic_titles_are_refused_with_or_without_accents_and_case() {
        for given in [
            "New Tab",
            "NOUVEL ONGLET",
            "Google Chrome",
            "chrome",
            "Microsoft Edge",
            "Mozilla Firefox",
            "about:blank",
            "Sans titre",
            "Nouvelle Fen\u{ea}tre",
            "nouvelle fenetre",
            "New Window",
            "Untitled",
            "Vivaldi",
            "Brave",
            "Opera",
        ] {
            assert!(usage(&TargetRequirement::create("chrome", given)), "{given}");
        }
    }

    #[test]
    fn fragments_of_generic_titles_are_refused_but_specific_titles_pass() {
        assert!(usage(&TargetRequirement::create("chrome", "nouv")));
        assert!(usage(&TargetRequirement::create("chrome", "w ta")));
        assert!(TargetRequirement::create("chrome", "Ma banque - connexion").is_ok());
        assert!(TargetRequirement::create("firefox", "Example Domain").is_ok());
    }

    #[test]
    fn matching_ignores_case_but_needs_the_exact_process() {
        let requirement = TargetRequirement::create("chrome.exe", "Ma Banque").unwrap();
        assert!(requirement.matches("CHROME", "Connexion - ma banque - Google Chrome"));
        assert!(requirement.matches("chrome", "MA BANQUE"));
        assert!(!requirement.matches("chromium", "Ma banque"));
        assert!(!requirement.matches("chrome", "Ma bank"));
        assert!(!requirement.matches("chrome", ""));
    }

    #[test]
    fn matching_does_not_fold_accents_in_titles() {
        let requirement = TargetRequirement::create("chrome", "Caf\u{e9} noir").unwrap();
        assert!(requirement.matches("chrome", "Caf\u{e9} noir"));
        assert!(!requirement.matches("chrome", "Cafe noir"));
    }
}
