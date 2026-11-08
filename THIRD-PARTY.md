# Licences et provenance

Le service Windows, l'interface, le helper JavaScript et l'installateur propres à VaultAgent sont
distribués sous la licence MIT du dépôt. Cette licence ne remplace aucune licence des composants tiers.

L'adaptation `fork/agent.ts` et le moteur CLI compilé s'appuient sur Bitwarden CLI, sous GPLv3 :
voir `LICENSE-GPL.txt` et les mentions conservées dans les sources et dépendances.

- Sources : https://github.com/bitwarden/clients
- Révision : `91214c3b978edb5496a32a946451960fe7d44531`
- Tag : `cli-v2026.10.1`
- Adaptation : `fork/agent.ts` et `fork/build.cjs`
- Reconstruction : `build-engine.ps1`, puis `prepare-package.ps1` et `build-installer.ps1`

Le paquet contient l'adaptation et les scripts dans `sources`, la révision dans
`runtime/engine/upstream.json`, ainsi que les licences conservées dans les dépendances Node.
Les sources correspondantes du moteur sont obtenues à la révision exacte par `build-engine.ps1`.
Le code des algorithmes de chiffrement et de dérivation reste celui des composants amont.

Node.js et le runtime Microsoft .NET embarqués conservent leurs propres licences. Les dépendances
NuGet et npm conservent leurs mentions. VaultAgent n'est pas un produit officiel Bitwarden.
