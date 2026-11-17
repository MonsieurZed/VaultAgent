//! Pagination de la liste des appareils : 8 lignes par page, moins quand des demandes attendent (minimum 3).

const FULL_PAGE: usize = 8;
const MIN_PAGE: usize = 3;

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
    /// Paramètres : le nombre d'appareils, le nombre de lignes de demandes affichées, le paramètre `page` brut.
    /// Retour : la fenêtre ; une valeur absente, invalide ou hors plage donne la page la plus proche, jamais une erreur.
    #[must_use]
    pub fn new(total: usize, request_rows: usize, requested: Option<&str>) -> Self {
        let size = FULL_PAGE.saturating_sub(request_rows).max(MIN_PAGE);
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
    fn eight_per_page_without_requests() {
        let paging = Paging::new(20, 0, None);
        assert_eq!((paging.page, paging.count, paging.size), (1, 3, 8));
        assert_eq!(paging.range(20), 0..8);
        assert_eq!(Paging::new(20, 0, Some("3")).range(20), 16..20);
    }

    #[test]
    fn requests_shrink_the_page_down_to_three() {
        assert_eq!(Paging::new(20, 3, None).size, 5);
        assert_eq!(Paging::new(20, 5, None).size, 3);
        assert_eq!(Paging::new(20, 9, None).size, 3);
    }

    #[test]
    fn a_bad_page_is_clamped_never_an_error() {
        assert_eq!(Paging::new(20, 0, Some("0")).page, 1);
        assert_eq!(Paging::new(20, 0, Some("99")).page, 3);
        assert_eq!(Paging::new(20, 0, Some("abc")).page, 1);
        assert_eq!(Paging::new(20, 0, Some("-2")).page, 1);
        assert_eq!(
            Paging::new(0, 0, Some("4")),
            Paging {
                page: 1,
                count: 1,
                size: 8
            }
        );
        assert_eq!(Paging::new(0, 0, None).range(0), 0..0);
    }
}
