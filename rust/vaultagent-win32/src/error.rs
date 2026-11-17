//! Erreurs des enveloppes Win32 : une énumération, sans secret ni texte venu d'une fenêtre.

use std::fmt;

/// Échec d'un appel Win32 ou d'une lecture de fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Win32Error {
    /// Aucune fenêtre n'est au premier plan.
    NoForegroundWindow,
    /// Le processus propriétaire de la fenêtre n'est pas lisible.
    UnreadableProcess,
    /// L'utilisateur a annulé l'invite de contrôle de compte (`ERROR_CANCELLED`).
    Cancelled,
    /// Le processus élevé n'a pas fini dans le délai et a été tué.
    TimedOut,
    /// Argument refusé avant tout appel système (défi, RP ou origine qui exigerait un échappement JSON).
    InvalidArgument,
    /// Appel système refusé, avec son nom et le code de `GetLastError`.
    Api {
        /// Nom de la fonction Win32 appelée.
        call: &'static str,
        /// Code rendu par `GetLastError`.
        code: u32,
    },
}

impl fmt::Display for Win32Error {
    /// Écrit une phrase courte sans donnée de fenêtre.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoForegroundWindow => formatter.write_str("No window is in the foreground."),
            Self::UnreadableProcess => formatter.write_str("The process of the window is unreadable."),
            Self::Cancelled => formatter.write_str("The elevation prompt was cancelled."),
            Self::TimedOut => formatter.write_str("The elevated process did not finish in time."),
            Self::InvalidArgument => formatter.write_str("An argument is not accepted."),
            Self::Api { call, code } => write!(formatter, "{call} failed with Win32 error {code}."),
        }
    }
}

impl std::error::Error for Win32Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_call_and_the_code() {
        let error = Win32Error::Api {
            call: "SendInput",
            code: 5,
        };
        assert_eq!(error.to_string(), "SendInput failed with Win32 error 5.");
        assert!(Win32Error::NoForegroundWindow.to_string().contains("foreground"));
        assert!(Win32Error::UnreadableProcess.to_string().contains("process"));
        assert!(Win32Error::Cancelled.to_string().contains("cancelled"));
        assert!(Win32Error::TimedOut.to_string().contains("time"));
    }
}
