//! Pagination de la liste des appareils : un nombre fixe de lignes par page, dans le bloc « Appareils ».

const PAGE_SIZE: usize = 6;

/// Fenêtre de la liste affichée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paging {
    /// Page affichée (à partir de 1).
    pub page: usize,
    /// Nombre de pages (au moins 1).
    pub count: usize,
    /// Nombre de lignes par page.
    pub size: usize,
}

impl Paging {
    /// Calcule la fenêtre.
    ///
    /// Paramètres : le nombre d'appareils, le paramètre `page` brut.
    /// Retour : la fenêtre ; une valeur absente, invalide ou hors plage donne la page la plus proche, jamais une erreur.
    #[must_use]
    pub fn new(total: usize, requested: Option<&str>) -> Self {
        let size = PAGE_SIZE;
        let count = total.div_ceil(size).max(1);
        let page = requested
            .and_then(|text| text.trim().parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, count);
        Self { page, count, size }
    }

    /// Indices des appareils de la page.
    ///
    /// Paramètres : le nombre d'appareils.
    /// Retour : l'intervalle à afficher.
    #[must_use]
    pub fn range(&self, total: usize) -> std::ops::Range<usize> {
        let start = ((self.page - 1) * self.size).min(total);
        start..(start + self.size).min(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_per_page() {
        let paging = Paging::new(20, None);
        assert_eq!((paging.page, paging.count, paging.size), (1, 4, 6));
        assert_eq!(paging.range(20), 0..6);
        assert_eq!(Paging::new(20, Some("4")).range(20), 18..20);
    }

    #[test]
    fn a_bad_page_is_clamped_never_an_error() {
        assert_eq!(Paging::new(20, Some("0")).page, 1);
        assert_eq!(Paging::new(20, Some("99")).page, 4);
        assert_eq!(Paging::new(20, Some("abc")).page, 1);
        assert_eq!(Paging::new(20, Some("-2")).page, 1);
        assert_eq!(
            Paging::new(0, Some("4")),
            Paging {
                page: 1,
                count: 1,
                size: 6
            }
        );
        assert_eq!(Paging::new(0, None).range(0), 0..0);
    }
}
