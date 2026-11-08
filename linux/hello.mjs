/* Verification WebAuthn native : seule la cle publique est conservee sur Linux. */
import { createHash, createPublicKey, verify, constants, timingSafeEqual } from 'node:crypto';
export const helloRpId = 'vaultagent-client.local';
export const helloOrigin = 'https://' + helloRpId;
export const helloChallenge = message => createHash('sha256').update(message).digest('base64url');
function decode(value, maximum) {
  if (typeof value !== 'string' || value.length > maximum * 2) throw new Error('hello-invalid');
  const bytes = Buffer.from(value, 'base64');
  if (!bytes.length || bytes.length > maximum || bytes.toString('base64') !== value) throw new Error('hello-invalid');
  return bytes;
}
export function helloPublicKey(credential) {
  if (!credential || decode(credential.id, 4096).length < 16) throw new Error('hello-invalid');
  const first = decode(credential.first, 512); const second = decode(credential.second, 512);
  if (credential.keyType === 'EC2' && first.length === 32 && second.length === 32) return createPublicKey({ format: 'jwk', key: { kty: 'EC', crv: 'P-256', x: first.toString('base64url'), y: second.toString('base64url') } });
  if (credential.keyType === 'RSA' && first.length >= 256 && second.length <= 4) return createPublicKey({ format: 'jwk', key: { kty: 'RSA', n: first.toString('base64url'), e: second.toString('base64url') } });
  throw new Error('hello-invalid');
}
export function verifyHello(credential, proof, message) {
  const key = helloPublicKey(credential);
  if (!proof || typeof proof !== 'object') throw new Error('hello-required');
  const id = decode(proof.credentialId, 4096); const expectedId = decode(credential.id, 4096);
  if (id.length !== expectedId.length || !timingSafeEqual(id, expectedId)) throw new Error('hello-invalid');
  const clientData = decode(proof.clientData, 16384); const client = JSON.parse(clientData.toString('utf8'));
  if (client.type !== 'webauthn.get' || client.origin !== helloOrigin || client.challenge !== helloChallenge(message) || client.crossOrigin !== false) throw new Error('hello-invalid');
  const auth = decode(proof.authenticatorData, 4096);
  if (auth.length < 37 || !timingSafeEqual(auth.subarray(0, 32), createHash('sha256').update(helloRpId).digest()) || (auth[32] & 5) !== 5) throw new Error('hello-invalid');
  const signed = Buffer.concat([auth, createHash('sha256').update(clientData).digest()]);
  const options = credential.keyType === 'RSA' ? { key, padding: constants.RSA_PKCS1_PADDING } : { key, dsaEncoding: 'der' };
  if (!verify('sha256', signed, options, decode(proof.signature, 1024))) throw new Error('hello-invalid');
  return true;
}
