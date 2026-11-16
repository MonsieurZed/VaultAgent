//! Boucle de relecture des écritures : cinq essais espacés de 200 ms, l'attente étant injectée.

use vaultagent_core::ErrorCode;
use vaultagent_core::verify::{ATTEMPTS, RETRY_DELAY};

use crate::context::Context;
use crate::error::Result;
use crate::guard::Guard;

/// Relit jusqu'à ce que l'effet soit constaté.
///
/// Paramètres : le contexte (attente), la garde (contrôlée avant chaque essai), la relecture qui rend `Some` quand
/// l'effet est constaté.
/// Retour : la valeur constatée.
///
/// # Errors
///
/// `request-refused` si l'effet n'est pas constaté après tous les essais ; sinon l'erreur de la garde ou de la
/// relecture.
pub fn until_confirmed<T>(
    context: &Context,
    guard: &Guard<'_>,
    mut attempt: impl FnMut() -> Result<Option<T>>,
) -> Result<T> {
    let delay = u64::try_from(RETRY_DELAY.as_millis()).unwrap_or(200);
    for _ in 0..ATTEMPTS {
        guard.check()?;
        if let Some(found) = attempt()? {
            return Ok(found);
        }
        context.pause.sleep_ms(delay);
    }
    Err(ErrorCode::RequestRefused.into())
}
