/* Registre d'idempotence des écritures : une opération n'est jamais rejouée à l'aveugle. */
import { createHmac } from 'node:crypto';
import { canonical } from './write-checks.mjs';

const LEDGER_FILE = 'write-receipts.json';
const CAPACITY = 10000;

/**
 * Ouvre le registre et calcule l'empreinte de la requête.
 * @param {object} session Session du coffre.
 * @param {object} input Requête d'écriture validée.
 * @returns {Promise<object>} Entrée { ledger, key, fingerprint, previous }.
 */
export async function openLedger(session, input) {
  const ledger = await session.store.load(LEDGER_FILE, {});
  const key = session.store.account.profile + ':' + input.operationId;
  const fingerprint = createHmac('sha256', session.store.key).update(JSON.stringify(canonical(input))).digest('hex');
  return { ledger, key, fingerprint, previous: ledger[key] };
}

/**
 * Renvoie le reçu d'une opération déjà exécutée, ou null si elle est nouvelle.
 * @param {object} entry Entrée ouverte par openLedger.
 * @returns {object|null} Reçu marqué replayed, ou null.
 */
export function replayed(entry) {
  if (!entry.previous) return null;
  if (entry.previous.fingerprint !== entry.fingerprint) throw new Error('operation-conflict');
  if (!entry.previous.receipt) throw new Error('write-indeterminate');
  return { ...entry.previous.receipt, replayed: true };
}

/**
 * Refuse une nouvelle opération quand le registre est plein.
 * @param {object} entry Entrée ouverte par openLedger.
 * @returns {void}
 */
export function ensureCapacity(entry) {
  if (Object.keys(entry.ledger).length >= CAPACITY) throw new Error('receipt-capacity');
}

/**
 * Enregistre l'intention d'écrire avant toute requête d'écriture.
 * @param {object} session Session du coffre.
 * @param {object} entry Entrée ouverte par openLedger.
 * @returns {Promise<void>}
 */
export async function recordIntent(session, entry) {
  entry.ledger[entry.key] = { fingerprint: entry.fingerprint };
  await session.store.save(LEDGER_FILE, entry.ledger);
}

/**
 * Enregistre le reçu d'une écriture vérifiée.
 * @param {object} session Session du coffre.
 * @param {object} entry Entrée ouverte par openLedger.
 * @param {object} receipt Reçu à conserver et à renvoyer.
 * @returns {Promise<object>} Le reçu.
 */
export async function recordReceipt(session, entry, receipt) {
  entry.ledger[entry.key] = { fingerprint: entry.fingerprint, receipt };
  await session.store.save(LEDGER_FILE, entry.ledger);
  session.touch();
  return receipt;
}
