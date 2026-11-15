//! Logique pure de `VaultAgent` : formes et patchs d'écriture, registre d'idempotence, cycle de vie des fiches,
//! demandes d'association et de déverrouillage, signatures RSA-PSS, empreintes.
//! Ne touche ni fichier, ni réseau, ni horloge.

#![forbid(unsafe_code)]
