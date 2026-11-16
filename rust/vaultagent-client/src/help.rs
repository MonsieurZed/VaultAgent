//! Textes d'aide anglais de la ligne de commande : tableau des codes de sortie et un exemple par commande.

/// Tableau des codes de sortie, affiché à la fin de l'aide générale.
pub const EXIT_CODES: &str = "\
Exit codes:
  0  success
  1  usage error (bad arguments, missing or invalid configuration)
  2  refused (server refusal, pairing denied, server unreachable)
  3  target refused (window not matching; typing commands only)
  4  vault locked
  5  outcome unknown (timeout or cut after sending)

Output: stdout carries only the server JSON (or the sealed envelope);
errors are one JSON line on stderr: {\"version\":1,\"error\":\"<code>\",\"message\":\"<text>\"}.";

/// Exemple de `pair-request`.
pub const PAIR_REQUEST: &str = "Example:\n  vaultagent pair-request https://vault.example.test my-laptop";

/// Exemple de `status`.
pub const STATUS: &str = "Example:\n  vaultagent status";

/// Exemple de `vaults`.
pub const VAULTS: &str = "Example:\n  vaultagent vaults";

/// Exemple de `sharing`.
pub const SHARING: &str = "Example:\n  vaultagent sharing";

/// Exemple de `catalog`.
pub const CATALOG: &str = "Example:\n  vaultagent catalog mail";

/// Exemple de `preview`.
pub const PREVIEW: &str = "Example:\n  vaultagent preview 0000000a-0000-4000-8000-00000000000c";

/// Exemple de `create`.
pub const CREATE: &str = "Example (secrets only on stdin):\n  \
echo '{\"password\":\"s3cret\"}' | vaultagent create --operation-id <uuid> --metadata '{\"type\":1,\"name\":\"Mail\"}' --values-stdin";

/// Exemple de `update`.
pub const UPDATE: &str = "Example:\n  vaultagent update <item-id> --operation-id <uuid> --expected-revision <date> --metadata '{\"name\":\"Mail 2\"}'";

/// Exemple de `delete`.
pub const DELETE: &str = "Example:\n  vaultagent delete <item-id> --operation-id <uuid> --expected-revision <date>";

/// Exemple de `restore`.
pub const RESTORE: &str = "Example:\n  vaultagent restore <item-id> --operation-id <uuid> --expected-revision <date>";

/// Exemple de `folder`.
pub const FOLDER: &str = "Example:\n  vaultagent folder create Work --operation-id <uuid>";

/// Exemple de `folder create`.
pub const FOLDER_CREATE: &str = "Example:\n  vaultagent folder create Work --operation-id <uuid>";

/// Exemple de `folder rename`.
pub const FOLDER_RENAME: &str = "Example:\n  vaultagent folder rename <folder-id> Archive --operation-id <uuid>";

/// Exemple de `folder delete`.
pub const FOLDER_DELETE: &str = "Example:\n  vaultagent folder delete <folder-id> --operation-id <uuid>";

/// Exemple de `unlock`.
pub const UNLOCK: &str = "Example:\n  vaultagent unlock --wait 60";

/// Exemple de `lock`.
pub const LOCK: &str = "Example:\n  vaultagent lock";

/// Exemple de `seal`, avec la règle de non-divulgation.
pub const SEAL: &str = "\
The value never leaves the program in clear text: stdout receives the encrypted envelope\n\
{encryptedKey, nonce, tag, data}, which only the holder of the private key can open.\n\
Example:\n  vaultagent seal <item-id> password https://example.test --public-key <spki-base64>";
