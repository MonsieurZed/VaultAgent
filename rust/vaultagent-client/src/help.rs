//! Textes d'aide anglais de la ligne de commande : tableau des codes de sortie et un exemple par commande.

/// Tableau des codes de sortie, affiché à la fin de l'aide générale.
pub const EXIT_CODES: &str = "\
Exit codes:
  0  success
  1  usage error (bad arguments, missing or invalid configuration)
  2  refused (server refusal, pairing denied, server unreachable)
  3  target refused (window not matching; focus and type, report on stdout)
  4  vault locked
  5  outcome unknown (timeout or cut after sending)

Output: stdout carries only the server JSON (or the sealed envelope);
errors are one JSON line on stderr: {\"version\":1,\"error\":\"<code>\",\"message\":\"<text>\"}.";

/// Exemple de `pair-request`.
pub const PAIR_REQUEST: &str = "Example:
  vaultagent pair-request https://vault.example.test my-laptop

On Windows without --config the token is stored protected by DPAPI in %LOCALAPPDATA%\\VaultAgent;
--approver also registers this PC's approver key.";

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

/// Exemple de `approvals`.
pub const APPROVALS: &str =
    "Prints {\"approvalRequests\":[{vaultId, requestId, challenge, helloRequired, elevationRequired}]}.
Example:\n  vaultagent approvals";

/// Exemple de `decide`, avec la règle de la validation Windows.
pub const DECIDE: &str = "Signs the decision with this PC's approver key. Approving a request that requires the Windows
validation shows the Windows prompt (UAC); denying never does.
Example:\n  vaultagent decide <request-id> --approve --remember\n  vaultagent decide <request-id> --deny";

/// Exemple de `elevation-enroll`.
pub const ELEVATION_ENROLL: &str = "Shows one Windows prompt (UAC) per vault to enroll; prints {\"enrolled\":<n>}.
Example:\n  vaultagent elevation-enroll";

/// Exemple de `hello-status`.
pub const HELLO_STATUS: &str = "Prints {\"available\":<bool>,\"apiVersion\":<n>}; shows no Windows Hello prompt.
Example:
  vaultagent hello-status";

/// Exemple de `hello-enroll`.
pub const HELLO_ENROLL: &str = "Shows the Windows Hello prompt once to create the key, then once per vault to enroll;
prints {\"enrolled\":<n>}.
Example:
  vaultagent hello-enroll";

/// Exemple de `seal`, avec la règle de non-divulgation.
pub const SEAL: &str = "\
The value never leaves the program in clear text: stdout receives the encrypted envelope\n\
{encryptedKey, nonce, tag, data}, which only the holder of the private key can open.\n\
Example:\n  vaultagent seal <item-id> password https://example.test --public-key <spki-base64>";

/// Exemple de `foreground`.
pub const FOREGROUND: &str = "Prints {\"process\":\"<name>\",\"title\":\"<title>\"}; no server needed.
Example:
  vaultagent foreground";

/// Exemple de `focus`.
pub const FOCUS: &str = "Brings the single visible window matching --process and --title to the foreground.
Exit code 3 with {\"focused\":false,\"reason\":…,\"expected\":…,\"foreground\":…} when none or several match.
Example:
  vaultagent focus --process chrome --title \"My bank\"";

/// Exemple de `type`, avec la règle de non-divulgation et le code 3.
pub const TYPE: &str =
    "The value is typed character by character with SendInput, never copied to the clipboard, never printed.
The foreground window is checked before the request and before every character; exit code 3 with
{\"typed\":false,\"reason\":…,\"expected\":…,\"foreground\":…} when it is not the target.
Example:
  vaultagent type <item-id> password https://example.test --process chrome --title \"My bank\"";

/// Exemple de `tray`.
pub const TRAY: &str = "Runs until Quit; one instance per user session; no JSON output.
Example:
  vaultagent tray";
