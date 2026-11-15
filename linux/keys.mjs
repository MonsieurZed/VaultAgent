/**
 * Clés publiques RSA des PC approbateurs : validation et vérification des signatures RSA-PSS.
 */
import { createPublicKey, verify, constants } from 'node:crypto';

/**
 * Charge une clé publique RSA 2048 bits encodée en SPKI base64.
 * @param {string} publicKey Clé publique en base64.
 * @returns {import('node:crypto').KeyObject} Clé chargée.
 */
export function rsaPublicKey(publicKey) {
  if (typeof publicKey !== 'string' || publicKey.length > 1024) throw new Error('invalid-key');
  let key;
  try { key = createPublicKey({ key: Buffer.from(publicKey, 'base64'), type: 'spki', format: 'der' }); } catch { throw new Error('invalid-key'); }
  if (key.asymmetricKeyType !== 'rsa' || key.asymmetricKeyDetails.modulusLength !== 2048) throw new Error('invalid-key');
  return key;
}

/**
 * Vérifie une signature RSA-PSS SHA-256 et lève l'erreur donnée si elle est invalide.
 * @param {string} publicKey Clé publique en base64.
 * @param {string} message Message signé.
 * @param {string} signature Signature en base64.
 * @param {string} failure Code d'erreur si la signature est refusée.
 * @returns {void}
 */
export function verifyPss(publicKey, message, signature, failure) {
  if (typeof signature !== 'string' || !verify('sha256', Buffer.from(message), { key: rsaPublicKey(publicKey), padding: constants.RSA_PKCS1_PSS_PADDING, saltLength: 32 }, Buffer.from(signature, 'base64'))) throw new Error(failure);
}
