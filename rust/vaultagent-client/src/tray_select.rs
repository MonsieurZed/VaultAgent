//! Choix pur de l'application de barre : mode de validation (carte ou UAC), demande à présenter, texte d'état.

use std::collections::HashSet;

use crate::translate::Translator;
use crate::tray_status::{TrayRequest, VaultSnapshot};

/// Longueur maximale d'une info-bulle de barre, en caractères.
pub const TIP_LIMIT: usize = 63;

/// Mode de validation d'une demande.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Carte avec « Oui », « Non » et « Toujours accepter », sans invite UAC.
    Card,
    /// Bulle d'information puis invite UAC directe ; l'annulation refuse.
    Uac,
}

/// Déduit le mode du serveur.
///
/// Paramètres : `approvalSecurity.elevationEnabled` du coffre.
/// Retour : `Uac` si l'élévation est active, sinon `Card`.
#[must_use]
pub fn mode_for(elevation_enabled: bool) -> ValidationMode {
    if elevation_enabled {
        ValidationMode::Uac
    } else {
        ValidationMode::Card
    }
}

/// Mode global du poste.
///
/// Paramètres : les états des coffres.
/// Retour : `Uac` si un coffre approuvé par ce poste a l'élévation active, `Card` s'il y a des coffres sans cela,
/// `None` sans coffre connu.
#[must_use]
pub fn overall_mode(snapshots: &[VaultSnapshot]) -> Option<ValidationMode> {
    if snapshots.is_empty() {
        return None;
    }
    Some(mode_for(snapshots.iter().any(|vault| vault.elevation_enabled)))
}

/// Demande à présenter à l'utilisateur.
#[derive(Debug, Clone, PartialEq)]
pub struct Presentation {
    /// La demande.
    pub request: TrayRequest,
    /// Le mode de son coffre.
    pub mode: ValidationMode,
}

/// Choisit la prochaine demande à présenter automatiquement.
///
/// Paramètres : les états, l'ensemble des demandes déjà notifiées (réduit à celles encore en attente), l'heure
/// courante et vrai si une carte ou une décision est déjà en cours.
/// Retour : la première demande non échue et jamais notifiée, marquée notifiée ; `None` sinon.
pub fn pick_new<S: std::hash::BuildHasher>(
    snapshots: &[VaultSnapshot],
    notified: &mut HashSet<String, S>,
    now: i64,
    busy: bool,
) -> Option<Presentation> {
    let pending: HashSet<String> = snapshots
        .iter()
        .flat_map(|vault| vault.requests.iter().map(TrayRequest::key))
        .collect();
    notified.retain(|key| pending.contains(key));
    if busy {
        return None;
    }
    let found = latest_with(snapshots, now, |request| !notified.contains(&request.key()), false)?;
    notified.insert(found.request.key());
    Some(found)
}

/// Retrouve une demande à rouvrir depuis le menu.
///
/// Paramètres : les états et l'heure courante.
/// Retour : la dernière demande non échue de la liste, `None` s'il n'y en a pas.
#[must_use]
pub fn latest_open(snapshots: &[VaultSnapshot], now: i64) -> Option<Presentation> {
    latest_with(snapshots, now, |_| true, true)
}

/// Parcourt les demandes non échues et en retient une.
///
/// Paramètres : les états, l'heure, le filtre et vrai pour retenir la dernière, faux pour la première.
/// Retour : la demande retenue avec le mode de son coffre.
fn latest_with(
    snapshots: &[VaultSnapshot],
    now: i64,
    keep: impl Fn(&TrayRequest) -> bool,
    last: bool,
) -> Option<Presentation> {
    let mut candidates = snapshots.iter().flat_map(|vault| {
        let mode = mode_for(vault.elevation_enabled);
        vault
            .requests
            .iter()
            .filter(|request| !request.is_expired(now) && keep(request))
            .map(move |request| Presentation {
                request: request.clone(),
                mode,
            })
    });
    if last { candidates.last() } else { candidates.next() }
}

/// Situation résumée par le texte d'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overview {
    /// Aucune association.
    NotPaired,
    /// Serveur injoignable ou réponse refusée.
    Unavailable,
    /// Coffres lus : nombre déverrouillés et total.
    Vaults {
        /// Coffres déverrouillés.
        unlocked: usize,
        /// Coffres connus.
        total: usize,
    },
}

/// Résume l'état des coffres.
///
/// Paramètres : les états.
/// Retour : `Vaults` avec les comptes.
#[must_use]
pub fn overview_of(snapshots: &[VaultSnapshot]) -> Overview {
    Overview::Vaults {
        unlocked: snapshots.iter().filter(|vault| vault.unlocked).count(),
        total: snapshots.len(),
    }
}

/// Écrit le texte d'état du menu.
///
/// Paramètres : le traducteur et la situation.
/// Retour : le texte, préfixé de `déverrouillés/total · ` quand il y a plusieurs coffres.
#[must_use]
pub fn status_text(tr: &Translator, overview: Overview) -> String {
    match overview {
        Overview::NotPaired => tr.t("Client à associer"),
        Overview::Unavailable => tr.t("Service du coffre indisponible"),
        Overview::Vaults { unlocked, total } => {
            let prefix = if total > 1 {
                format!("{unlocked}/{total} · ")
            } else {
                String::new()
            };
            let key = if unlocked > 0 {
                "Coffre disponible pour les clients"
            } else {
                "Coffre verrouillé"
            };
            format!("{prefix}{}", tr.t(key))
        }
    }
}

/// Tronque un texte pour l'info-bulle de barre.
///
/// Paramètres : le texte.
/// Retour : au plus `TIP_LIMIT` caractères.
#[must_use]
pub fn tip_of(text: &str) -> String {
    text.chars().take(TIP_LIMIT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::Language;
    use crate::tray_status::parse_snapshot;

    /// Coffre de test avec ses demandes (identifiant, échéance).
    fn vault(id: &str, elevation: bool, requests: &[(&str, &str)]) -> VaultSnapshot {
        let list: Vec<String> = requests
            .iter()
            .map(|(id, end)| {
                format!("{{\"requestId\":\"{id}\",\"challenge\":\"c\",\"deviceName\":\"PC\",\"expiresUtc\":\"{end}\"}}")
            })
            .collect();
        let body = format!(
            "{{\"unlocked\":false,\"approvalSecurity\":{{\"elevationEnabled\":{elevation}}},\"approvalRequests\":[{}]}}",
            list.join(",")
        );
        parse_snapshot(id, &body).unwrap()
    }

    const LATER: &str = "2030-01-01T00:00:00Z";
    const PAST: &str = "1970-01-01T00:00:10Z";

    #[test]
    fn mode_follows_the_server() {
        assert_eq!(mode_for(true), ValidationMode::Uac);
        assert_eq!(mode_for(false), ValidationMode::Card);
        assert_eq!(overall_mode(&[]), None);
        let both = [vault("a", false, &[]), vault("b", true, &[])];
        assert_eq!(overall_mode(&both), Some(ValidationMode::Uac));
        assert_eq!(overall_mode(&both[..1]), Some(ValidationMode::Card));
    }

    #[test]
    fn a_request_is_presented_once_and_with_its_vault_mode() {
        let snapshots = [vault("a", false, &[("r1", LATER)]), vault("b", true, &[("r2", LATER)])];
        let mut notified = HashSet::new();
        let first = pick_new(&snapshots, &mut notified, 100, false).unwrap();
        assert_eq!(
            (first.request.request.request_id.as_str(), first.mode),
            ("r1", ValidationMode::Card)
        );
        let second = pick_new(&snapshots, &mut notified, 100, false).unwrap();
        assert_eq!(
            (second.request.request.request_id.as_str(), second.mode),
            ("r2", ValidationMode::Uac)
        );
        assert!(pick_new(&snapshots, &mut notified, 100, false).is_none());
    }

    #[test]
    fn busy_expired_and_gone_requests_are_not_presented() {
        let snapshots = [vault("a", false, &[("r1", PAST), ("r2", LATER)])];
        let mut notified = HashSet::new();
        assert!(pick_new(&snapshots, &mut notified, 100, true).is_none());
        let picked = pick_new(&snapshots, &mut notified, 100, false).unwrap();
        assert_eq!(picked.request.request.request_id, "r2");
        notified.insert("zz/old".to_owned());
        assert!(pick_new(&[vault("a", false, &[])], &mut notified, 100, false).is_none());
        assert!(notified.is_empty());
    }

    #[test]
    fn menu_reopens_the_last_unexpired_request() {
        let snapshots = [vault("a", false, &[("r1", LATER), ("r2", LATER), ("r3", PAST)])];
        assert_eq!(latest_open(&snapshots, 100).unwrap().request.request.request_id, "r2");
        assert!(latest_open(&snapshots[..0], 100).is_none());
    }

    #[test]
    fn status_text_counts_vaults() {
        let tr = Translator::new(Language::French);
        assert_eq!(status_text(&tr, Overview::NotPaired), "Client à associer");
        assert_eq!(
            status_text(&tr, Overview::Unavailable),
            "Service du coffre indisponible"
        );
        let one = overview_of(&[vault("a", false, &[])]);
        assert_eq!(status_text(&tr, one), "Coffre verrouillé");
        let many = Overview::Vaults { unlocked: 1, total: 3 };
        assert_eq!(status_text(&tr, many), "1/3 · Coffre disponible pour les clients");
        assert_eq!(tip_of(&"é".repeat(100)).chars().count(), TIP_LIMIT);
    }
}
