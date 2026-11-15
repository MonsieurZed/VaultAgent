//! Seule crate de `VaultAgent` où `unsafe` est autorisé (déclaré dans son `Cargo.toml`) : identité et premier plan
//! des fenêtres, `SendInput`, DPAPI, clés `NCrypt`, WebAuthn Hello, icône de barre et carte d'approbation.
//! Elle n'expose que des fonctions sûres ; chaque bloc `unsafe` porte un docblock `Sûreté :`.

#![cfg(windows)]
#![deny(unsafe_op_in_unsafe_fn)]
