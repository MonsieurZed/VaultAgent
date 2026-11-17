//! Arguments `clap` des commandes de fenêtre : cible (processus et titre) de `focus` et de `type`.

use clap::Args;

/// Cible d'une fenêtre : processus exact et fragment de titre.
#[derive(Debug, Clone, Args)]
pub struct WindowArgs {
    /// Nom du processus, avec ou sans `.exe`.
    #[arg(
        long,
        value_name = "EXE",
        help = "Process name of the target window, with or without .exe (for example chrome)."
    )]
    pub process: String,
    /// Fragment de titre qui doit figurer dans la fenêtre.
    #[arg(
        long,
        value_name = "FRAGMENT",
        help = "Fragment of the window title (3 characters at least, not a generic browser title)."
    )]
    pub title: String,
}

/// Arguments de `type`.
#[derive(Debug, Clone, Args)]
pub struct TypeArgs {
    /// Identifiant (UUID) de la fiche.
    #[arg(help = "Item id (UUID).")]
    pub item_id: String,
    /// Champ à taper : `username`, `password` ou `totp`.
    #[arg(help = "Field to type: username, password or totp.")]
    pub field: String,
    /// Origine HTTPS exacte de la cible.
    #[arg(help = "Exact HTTPS origin of the target page, without path.")]
    pub origin: String,
    /// Fenêtre cible.
    #[command(flatten)]
    pub window: WindowArgs,
}
