# Espace de travail Rust de VaultAgent

Réécriture en Rust de VaultAgent (plan : [`../docs/rust/plan.md`](../docs/rust/plan.md)). Les crates sont vides à ce
stade (phase 0) : aucune dépendance externe. Les décisions d'architecture sont dans [`../docs/decisions/`](../docs/decisions/) ;
les dépendances prévues et leurs justifications dans [`DEPENDANCES.md`](DEPENDANCES.md).

| Crate | Rôle |
|---|---|
| `vaultagent-core` | Logique pure, sans fichier, réseau ni horloge. |
| `vaultagent-engine` | Processus enfant Node, protocole JSON ligne à ligne. |
| `vaultagent-server` | Magasin chiffré, session, routes d'API (Linux). |
| `vaultagent-client` | Binaire `vaultagent` : association, lecture, écriture, remplissage. |
| `vaultagent-win32` | Seule crate autorisée à utiliser `unsafe` (Windows uniquement), déclarée dans son `Cargo.toml`. |

## Commandes

Depuis `rust/` :

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
conduite-check --crate rust/<crate> --outils
```

La dernière se lance depuis la racine du dépôt VaultAgent. Version de Rust fixée par `rust-toolchain.toml`.
