//! Processus enfant Node de `VaultAgent` : protocole JSON ligne à ligne de `fork/agent.ts`
//! (op `call`, `safe-update`, `sharing-*`), délais et redémarrage.

#![forbid(unsafe_code)]
