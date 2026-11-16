//! Dossier temporaire unique des tests, supprimé à la destruction (aucune dépendance de test).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Dossier créé sous le dossier temporaire du système.
pub struct TestDir(PathBuf);

impl TestDir {
    /// Crée un dossier au nom unique (processus, horloge, compteur).
    ///
    /// Retour : le dossier, supprimé avec son contenu quand la valeur est détruite.
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("vaultagent-client-{}-{nanos}-{count}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap_or_default();
        Self(path)
    }

    /// Chemin du dossier.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    /// Supprime le dossier et son contenu.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

/// Jeton factice de la forme du protocole, construit à l'exécution.
///
/// Retour : `va1.<uuid factice>.<43 fois A>`.
pub fn sample_token() -> String {
    format!("va1.0000000a-0000-4000-8000-00000000000b.{}", "A".repeat(43))
}
