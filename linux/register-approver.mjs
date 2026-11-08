/* Administration locale du serveur : cle publique uniquement, aucun jeton n'est exporte. */
import { createPublicKey } from 'node:crypto';
import { Store } from './store.mjs';
let input = ''; for await (const part of process.stdin) { input += part; if (input.length > 4096) throw new Error('input-size'); }
const { deviceId, publicKey } = JSON.parse(input);
if (!/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(deviceId || '') || typeof publicKey !== 'string') throw new Error('invalid-approver');
const key = createPublicKey({ key: Buffer.from(publicKey, 'base64'), type: 'spki', format: 'der' });
if (key.asymmetricKeyType !== 'rsa' || key.asymmetricKeyDetails.modulusLength !== 2048) throw new Error('invalid-key');
const store = await new Store(process.env.VAULTAGENT_DATA || '/data').init();
if (!store.devices.some(device => device.id === deviceId && device.profile === store.account?.profile)) throw new Error('device-missing');
await store.updateDevices(current => current.map(device => device.id === deviceId ? { ...device, approverPublicKey: publicKey } : device));
console.log(JSON.stringify({ registered: true, deviceId }));
