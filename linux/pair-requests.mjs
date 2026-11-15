/**
 * Demandes d'association approuvées depuis l'interface web du serveur.
 * Le client reçoit un code de contrôle court à comparer avec celui affiché sur la page d'administration,
 * puis attend l'approbation du propriétaire. Aucun code d'association n'est saisi sur le client.
 */
import { randomBytes, randomInt } from 'node:crypto';
import { digest, same } from './store.mjs';
import { rsaPublicKey } from './keys.mjs';

const ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
const LIFETIME_MS = 300000;
const MAX_PENDING = 8;
const MAX_OPENINGS_PER_MINUTE = 10;

export class PairRequests {
  constructor(clock = () => Date.now()) { this.clock = clock; this.entries = new Map(); this.openings = []; }

  purge() {
    for (const [id, entry] of this.entries) if (entry.expires < this.clock()) this.entries.delete(id);
  }

  /**
   * Ouvre une demande côté client, sans authentification.
   * @param {string} name Nom proposé pour l'appareil.
   * @param {string} [approverPublicKey] Clé publique du PC approbateur, enregistrée si le propriétaire autorise.
   * @returns {{requestId: string, secret: string, verification: string, expiresInSeconds: number}} Identifiants de la demande.
   */
  open(name, approverPublicKey) {
    const clean = typeof name === 'string' ? name.replace(/[\u0000-\u001f\u007f]/g, '').trim() : '';
    if (!clean || clean.length > 80) throw new Error('invalid-device');
    if (approverPublicKey !== undefined) rsaPublicKey(approverPublicKey);
    this.purge();
    this.openings = this.openings.filter(time => time > this.clock() - 60000);
    if (this.openings.length >= MAX_OPENINGS_PER_MINUTE) throw new Error('pairing-rate-limit');
    if (this.entries.size >= MAX_PENDING) throw new Error('pairing-limit');
    this.openings.push(this.clock());
    const code = Array.from({ length: 8 }, () => ALPHABET[randomInt(ALPHABET.length)]).join('');
    const verification = code.slice(0, 4) + '-' + code.slice(4);
    const requestId = randomBytes(16).toString('base64url'); const secret = randomBytes(32).toString('base64url');
    this.entries.set(requestId, { name: clean, verification, approverPublicKey, secret: digest(secret), state: 'pending', expires: this.clock() + LIFETIME_MS });
    return { requestId, secret, verification, expiresInSeconds: LIFETIME_MS / 1000 };
  }

  /**
   * Liste les demandes en attente pour la page d'administration.
   * @returns {Array<{requestId: string, name: string, verification: string, expiresInSeconds: number}>} Demandes ouvertes.
   */
  list() {
    this.purge();
    return [...this.entries].filter(([, entry]) => entry.state === 'pending')
      .map(([requestId, entry]) => ({ requestId, name: entry.name, verification: entry.verification, approver: !!entry.approverPublicKey, expiresInSeconds: Math.max(0, Math.round((entry.expires - this.clock()) / 1000)) }));
  }

  /**
   * Approuve une demande et crée l'appareil dans le coffre choisi.
   * @param {string} requestId Identifiant de la demande.
   * @param {{id: string, devices: {enrollNamed: Function}}} context Coffre qui reçoit l'appareil.
   * @returns {Promise<void>}
   */
  async approve(requestId, context) {
    this.purge();
    const entry = this.entries.get(requestId);
    if (!entry || entry.state !== 'pending') throw new Error('request-expired');
    entry.state = 'approving';
    try { entry.result = { ...await context.devices.enrollNamed(entry.name, entry.approverPublicKey), vaultId: context.id }; entry.state = 'approved'; }
    catch (error) { entry.state = 'pending'; throw error; }
  }

  /**
   * Refuse une demande en attente.
   * @param {string} requestId Identifiant de la demande.
   * @returns {void}
   */
  deny(requestId) {
    const entry = this.entries.get(requestId);
    if (!entry || entry.state !== 'pending') throw new Error('request-expired');
    entry.state = 'denied';
  }

  /**
   * Lit l'état d'une demande côté client. Le jeton n'est remis qu'une fois.
   * @param {string} requestId Identifiant de la demande.
   * @param {string} secret Secret remis à l'ouverture.
   * @returns {{state: string, token?: string, deviceId?: string, vaultId?: string, expiresInSeconds?: number}} État courant.
   */
  poll(requestId, secret) {
    this.purge();
    const entry = this.entries.get(String(requestId || ''));
    if (!entry || !same(entry.secret, digest(String(secret || '')))) throw new Error('pairing-refused');
    if (entry.state === 'approved') { this.entries.delete(requestId); return { state: 'approved', ...entry.result }; }
    if (entry.state === 'denied') { this.entries.delete(requestId); return { state: 'denied' }; }
    return { state: 'pending', expiresInSeconds: Math.max(0, Math.round((entry.expires - this.clock()) / 1000)) };
  }
}
