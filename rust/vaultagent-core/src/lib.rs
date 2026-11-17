//! Logique pure de `VaultAgent` : formes et patchs d'écriture, registre d'idempotence, cycle de vie des fiches,
//! demandes d'association et de déverrouillage, signatures RSA-PSS, empreintes.
//! Ne touche ni fichier, ni réseau, ni horloge.

#![forbid(unsafe_code)]

pub mod approval_policy;
pub mod approver_key;
pub mod canonical;
pub mod canonical_json;
pub mod creation_checks;
pub mod device_list;
pub mod device_record;
pub mod device_token;
pub mod devices;
pub mod encoding;
pub mod entropy;
pub mod envelope;
pub mod error;
pub mod fingerprint;
pub mod hello;
pub mod https;
pub mod jsval;
pub mod ledger;
pub mod ledger_flow;
pub mod lifecycle;
pub mod name;
pub mod new_item;
pub mod pair_requests;
pub mod pair_types;
pub mod patch;
pub mod patch_uris;
pub mod private_key;
pub mod projection;
pub mod protect;
pub mod request;
mod request_actions;
pub mod revision;
pub mod rsa_keys;
pub mod session_control;
pub mod share_checks;
pub mod share_flow;
pub mod share_request;
pub mod share_verify;
pub mod unlock_contexts;
pub mod unlock_decision;
pub mod unlock_elevation;
pub mod unlock_inputs;
pub mod unlock_replies;
pub mod unlock_requests;
pub mod uuid;
pub mod vault_clients;
pub mod vault_registry;
pub mod vault_select;
pub mod verify;

pub use approval_policy::ApprovalPolicy;
pub use approver_key::{decision_message, rsa_public_key, verify_pss};
pub use canonical::canonical;
pub use canonical_json::canonical_json;
pub use device_record::{DeviceRecord, HelloCredential, PublicDevice};
pub use devices::{Devices, Enrolled, IssuedCode};
pub use encoding::{digest, same};
pub use entropy::{Entropy, EntropyRng, SeededEntropy, SystemEntropy};
pub use envelope::{Envelope, open_envelope, seal};
pub use error::{ErrorCode, Result};
pub use fingerprint::fingerprint;
pub use hello::{HELLO_ORIGIN, HELLO_RP_ID, HelloKey, HelloProof, hello_challenge, hello_public_key, verify_hello};
pub use https::{HttpsAddress, https_origin};
pub use ledger::{CAPACITY, Ledger, LedgerEntry, Receipt, ReceiptStore, Registry};
pub use lifecycle::{Observed, SharingScope};
pub use name::BoundedName;
pub use new_item::{ItemKind, NewItem, NewLogin};
pub use pair_requests::PairRequests;
pub use pair_types::{Listed, Opened, Polled, Target};
pub use patch::apply_write_patch;
pub use private_key::{export_pkcs8, import_pkcs8};
pub use projection::{
    CatalogEntry, ItemDetail, ItemMetadata, Presence, active, catalog_entry, item_detail, item_metadata, visible,
};
pub use protect::{Protected, protect, unprotect};
pub use request::{WriteAction, WriteRequest};
pub use revision::RevisionDate;
pub use rsa::RsaPrivateKey;
pub use rsa_keys::{check_signature, generate_rsa, public_key_spki, sign_decision, sign_pss};
pub use session_control::SessionControl;
pub use share_checks::SharePreview;
pub use share_flow::ShareStart;
pub use share_request::{ShareAction, ShareRequest};
pub use unlock_inputs::{ElevationDisableInput, ElevationEnrollInput, HelloEnrollInput, SignedDecisionInput};
pub use unlock_replies::{DecisionReply, Outcome, PublicRequest, RequestState, UnlockReply, UnlockStatus};
pub use unlock_requests::{Env, UnlockRequests};
pub use uuid::Uuid;
pub use vault_clients::{AccountRef, Client, Grant};
pub use vault_registry::{SavedVault, VaultEntry, VaultRegistry};
pub use vault_select::{VaultDevices, VaultMetadata};
