//! Modèle pur du menu de l'icône de barre : identifiants, libellés traduits et entrées désactivées selon l'état.

use crate::translate::Translator;
use crate::tray_select::ValidationMode;

/// Identifiant d'une entrée de menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    /// Texte d'état, jamais cliquable.
    State = 1,
    /// Rouvre la demande de déverrouillage en attente.
    Request,
    /// Ouvre le serveur dans le navigateur.
    OpenServer,
    /// Ouvre la fenêtre d'association.
    Pair,
    /// Bascule la validation entre carte et UAC.
    Validation,
    /// Active Windows Hello.
    Hello,
    /// Verrouille les coffres.
    Lock,
    /// Désactive l'acceptation automatique.
    ResetAutomatic,
    /// Quitte l'application.
    Quit,
}

impl MenuId {
    /// Retrouve un identifiant rendu par le menu.
    ///
    /// Paramètres : la valeur numérique.
    /// Retour : l'identifiant, ou `None` s'il est inconnu.
    #[must_use]
    pub fn from_u16(value: u16) -> Option<Self> {
        [
            Self::State,
            Self::Request,
            Self::OpenServer,
            Self::Pair,
            Self::Validation,
            Self::Hello,
            Self::Lock,
            Self::ResetAutomatic,
            Self::Quit,
        ]
        .into_iter()
        .find(|id| *id as u16 == value)
    }
}

/// Ligne du menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuLine {
    /// Entrée.
    Item {
        /// Identifiant.
        id: MenuId,
        /// Libellé traduit.
        text: String,
        /// Faux si grisée.
        enabled: bool,
    },
    /// Séparateur.
    Separator,
}

/// Ce que le menu doit savoir de l'état courant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuState {
    /// Texte d'état déjà traduit.
    pub status: String,
    /// Vrai si une demande non échue est en attente.
    pub has_request: bool,
    /// Vrai si ce client est associé.
    pub paired: bool,
    /// Mode de validation courant, `None` s'il est inconnu.
    pub mode: Option<ValidationMode>,
}

/// Construit les lignes du menu dans l'ordre voulu.
///
/// Paramètres : le traducteur et l'état.
/// Retour : état, demande, ouverture du serveur, association, validation, Hello, verrouillage, acceptation
/// automatique, séparateur, quitter ; les entrées sans objet sont grisées.
#[must_use]
pub fn menu_lines(tr: &Translator, state: &MenuState) -> Vec<MenuLine> {
    let validation = match state.mode {
        Some(ValidationMode::Card) => "Validation : carte (passer à UAC)",
        Some(ValidationMode::Uac) => "Validation : UAC (passer à carte)",
        None => "Validation : indisponible",
    };
    let item = |id: MenuId, key: &str, enabled: bool| MenuLine::Item {
        id,
        text: tr.t(key),
        enabled,
    };
    vec![
        MenuLine::Item {
            id: MenuId::State,
            text: state.status.clone(),
            enabled: false,
        },
        item(MenuId::Request, "Demande de déverrouillage", state.has_request),
        item(MenuId::OpenServer, "Ouvrir le serveur VaultAgent", state.paired),
        item(MenuId::Pair, "Associer ce client", true),
        item(MenuId::Validation, validation, state.mode.is_some()),
        item(MenuId::Hello, "Activer Windows Hello", state.paired),
        item(MenuId::Lock, "Verrouiller maintenant", state.paired),
        item(
            MenuId::ResetAutomatic,
            "Désactiver l'acceptation automatique",
            state.paired,
        ),
        MenuLine::Separator,
        item(MenuId::Quit, "Quitter", true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::Language;

    /// État de menu de test.
    fn state(paired: bool, request: bool, mode: Option<ValidationMode>) -> MenuState {
        MenuState {
            status: "État".to_owned(),
            has_request: request,
            paired,
            mode,
        }
    }

    /// Indique si l'entrée est présente et active.
    fn enabled(lines: &[MenuLine], wanted: MenuId) -> bool {
        lines
            .iter()
            .any(|line| matches!(line, MenuLine::Item { id, enabled: true, .. } if *id == wanted))
    }

    #[test]
    fn menu_has_the_expected_order() {
        let tr = Translator::new(Language::French);
        let lines = menu_lines(&tr, &state(true, true, Some(ValidationMode::Card)));
        let ids: Vec<Option<MenuId>> = lines
            .iter()
            .map(|line| match line {
                MenuLine::Item { id, .. } => Some(*id),
                MenuLine::Separator => None,
            })
            .collect();
        assert_eq!(ids.len(), 10);
        assert_eq!(ids[0], Some(MenuId::State));
        assert_eq!(ids[8], None);
        assert_eq!(ids[9], Some(MenuId::Quit));
        assert!(!enabled(&lines, MenuId::State));
    }

    #[test]
    fn entries_follow_pairing_request_and_mode() {
        let tr = Translator::new(Language::French);
        let unpaired = menu_lines(&tr, &state(false, false, None));
        for id in [
            MenuId::Request,
            MenuId::OpenServer,
            MenuId::Validation,
            MenuId::Hello,
            MenuId::Lock,
        ] {
            assert!(!enabled(&unpaired, id), "{id:?}");
        }
        assert!(enabled(&unpaired, MenuId::Pair) && enabled(&unpaired, MenuId::Quit));
        let uac = menu_lines(&tr, &state(true, true, Some(ValidationMode::Uac)));
        assert!(enabled(&uac, MenuId::Request) && enabled(&uac, MenuId::Validation));
        let texts: Vec<&String> = uac
            .iter()
            .filter_map(|line| match line {
                MenuLine::Item {
                    id: MenuId::Validation,
                    text,
                    ..
                } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Validation : UAC (passer à carte)"]);
    }

    #[test]
    fn identifiers_round_trip() {
        assert_eq!(MenuId::from_u16(MenuId::Lock as u16), Some(MenuId::Lock));
        assert_eq!(MenuId::from_u16(0), None);
        assert_eq!(MenuId::from_u16(99), None);
    }
}
