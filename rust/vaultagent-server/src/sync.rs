//! Verrou de `Mutex` qui ignore l'empoisonnement : un fil en échec ne doit pas bloquer le service.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Prend un verrou même si un autre fil a paniqué en le tenant.
///
/// Paramètres : le verrou.
/// Retour : la garde.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
