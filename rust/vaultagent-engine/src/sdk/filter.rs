//! Filtrage et recherche des fiches : les mêmes règles que `linux/writes.mjs` (types 1 et 2, sans date d'archivage)
//! et la recherche de `bw list items --search` (nom, identifiant, notes et adresses, sans casse).

use crate::item::Item;

/// Indique si une fiche est exposée par le serveur : connexion ou note sécurisée, non archivée.
///
/// Paramètres : la fiche.
/// Retour : vrai si son type est 1 ou 2 et qu'elle n'a pas de date d'archivage.
#[must_use]
pub fn exposed(item: &Item) -> bool {
    matches!(item.kind, 1 | 2) && item.archived_date.is_none()
}

/// Indique si une fiche contient le texte cherché dans son nom, son identifiant de connexion, ses notes ou
/// l'une de ses adresses.
///
/// Paramètres : la fiche et le texte cherché (comparé sans tenir compte de la casse).
/// Retour : vrai si l'un des champs le contient.
#[must_use]
pub fn matches_search(item: &Item, search: &str) -> bool {
    let needle = search.to_lowercase();
    let contains = |text: &str| text.to_lowercase().contains(&needle);
    contains(&item.name)
        || item.notes.as_ref().is_some_and(|notes| contains(notes.expose()))
        || item.login.as_ref().is_some_and(|login| {
            login.username.as_deref().is_some_and(contains) || login.uris.iter().any(|entry| contains(&entry.uri))
        })
}

/// Fiches actives, filtrées par la recherche si elle est fournie.
///
/// Paramètres : toutes les fiches déchiffrées et le texte cherché.
/// Retour : les fiches exposées, hors corbeille, qui correspondent.
#[must_use]
pub fn active(items: Vec<Item>, search: Option<&str>) -> Vec<Item> {
    items
        .into_iter()
        .filter(|item| exposed(item) && item.deleted_date.is_none())
        .filter(|item| search.is_none_or(|text| matches_search(item, text)))
        .collect()
}

/// Fiches de la corbeille.
///
/// Paramètres : toutes les fiches déchiffrées.
/// Retour : les fiches exposées qui portent une date de mise en corbeille.
#[must_use]
pub fn trashed(items: Vec<Item>) -> Vec<Item> {
    items
        .into_iter()
        .filter(|item| exposed(item) && item.deleted_date.is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{Login, LoginUri};
    use crate::secret::Secret;

    /// Fiche factice.
    fn item(name: &str, kind: i64) -> Item {
        Item {
            id: name.into(),
            name: name.into(),
            kind,
            ..Item::default()
        }
    }

    #[test]
    fn only_types_one_and_two_without_archive_are_listed() {
        let mut archived = item("archive", 1);
        archived.archived_date = Some("2026-01-01T00:00:00Z".into());
        let mut trash = item("corbeille", 1);
        trash.deleted_date = Some("2026-01-01T00:00:00Z".into());
        let all = vec![
            item("login", 1),
            item("note", 2),
            item("carte", 3),
            archived,
            trash.clone(),
        ];
        let names: Vec<_> = active(all.clone(), None).into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["login", "note"]);
        assert_eq!(trashed(all), vec![trash]);
    }

    #[test]
    fn search_covers_name_user_notes_and_uris_without_case() {
        let mut entry = item("Banque", 1);
        entry.notes = Some(Secret::from("Code secret du guichet"));
        entry.login = Some(Login {
            username: Some("Alice".into()),
            uris: vec![LoginUri {
                uri: "https://Exemple.invalid/compte".into(),
            }],
            ..Login::default()
        });
        for hit in ["banq", "ALICE", "guichet", "exemple.invalid"] {
            assert!(matches_search(&entry, hit), "{hit}");
        }
        assert!(!matches_search(&entry, "absent"));
        assert_eq!(active(vec![entry], Some("absent")).len(), 0);
    }
}
