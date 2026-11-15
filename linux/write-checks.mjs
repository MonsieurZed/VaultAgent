/* Contrôles communs aux écritures : forme des données, identifiants et état d'une fiche. */

/**
 * Indique si une valeur est un UUID.
 * @param {unknown} value Valeur à tester.
 * @returns {boolean} Vrai pour un UUID bien formé.
 */
export const uuid = value => typeof value === 'string' && /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(value);

/**
 * Indique si une valeur est un objet simple (ni tableau ni null).
 * @param {unknown} value Valeur à tester.
 * @returns {boolean} Vrai pour un objet simple.
 */
export const plain = value => !!value && typeof value === 'object' && !Array.isArray(value);

/**
 * Indique si un objet simple ne contient que des clés autorisées.
 * @param {unknown} value Objet à tester.
 * @param {string[]} allowed Clés autorisées.
 * @returns {boolean} Vrai si toutes les clés sont autorisées.
 */
export const keys = (value, allowed) => plain(value) && Object.keys(value).every(key => allowed.includes(key));

/**
 * Indique si une valeur est une date ISO exploitable comme révision attendue.
 * @param {unknown} value Valeur à tester.
 * @returns {boolean} Vrai pour une date valide.
 */
export const revision = value => typeof value === 'string' && Number.isFinite(Date.parse(value));

/**
 * Indique si un nom de dossier ou de fiche est acceptable.
 * @param {unknown} value Nom proposé.
 * @param {number} max Longueur maximale.
 * @returns {boolean} Vrai si le nom est non vide, borné et sans caractère de contrôle.
 */
export const name = (value, max) => typeof value === 'string' && !!value.trim() && value.length <= max && !/[\x00-\x1f]/.test(value);

/**
 * Refuse une fiche archivée, d'un type non géré ou sans identifiant valide ; accepte la corbeille.
 * @param {object} item Fiche lue dans le coffre.
 * @returns {object} La même fiche.
 */
export function visible(item) {
  if (!uuid(item.id) || item.archivedDate || ![1, 2].includes(item.type)) throw new Error('item-refused');
  return item;
}

/**
 * Refuse en plus une fiche placée dans la corbeille.
 * @param {object} item Fiche lue dans le coffre.
 * @returns {object} La même fiche.
 */
export function active(item) {
  if (visible(item).deletedDate) throw new Error('item-refused');
  return item;
}

/**
 * Met un objet sous forme canonique : clés triées, valeurs nulles retirées.
 * @param {unknown} value Valeur à normaliser.
 * @returns {unknown} Copie normalisée.
 */
export function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (plain(value)) return Object.fromEntries(Object.keys(value).sort().filter(key => value[key] != null).map(key => [key, canonical(value[key])]));
  return value;
}
