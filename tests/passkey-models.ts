/* Verifier les conversions reelles du CLI avec une cle exclusivement factice. */
import "core-js/proposals/explicit-resource-management";
import { generateKeyPairSync, createPrivateKey, randomBytes, sign, verify } from "node:crypto";
import { CipherExport } from "@bitwarden/common/models/export/cipher.export";
import { CipherResponse } from "@agent-upstream/vault/models/cipher.response";
import { Fido2Utils } from "@bitwarden/common/platform/services/fido2/fido2-utils";

const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "prime256v1" });
const bytes = privateKey.export({ type: "pkcs8", format: "der" });
const credential = {
  credentialId: "b64." + randomBytes(32).toString("base64url"), keyType: "public-key",
  keyAlgorithm: "ECDSA", keyCurve: "P-256", keyValue: bytes.toString("base64url"),
  rpId: "example.test", rpName: "Site factice", userHandle: "AQIDBA",
  userName: "probe@example.invalid", userDisplayName: "Compte factice",
  counter: "0", discoverable: "true", creationDate: new Date().toISOString(),
};
const view = CipherExport.toView({ type: 1, name: "Test passkey", login: {
  username: credential.userName, password: null, totp: null, fido2Credentials: [credential],
} } as any);
view.id = "00000000-0000-4000-8000-000000000001";
const response = JSON.parse(JSON.stringify(new CipherResponse(view)));
const stored = response.login.fido2Credentials[0];
for (const field of Object.keys(credential)) {
  if (String(stored[field]) !== String((credential as any)[field])) throw new Error("Champ passkey perdu : " + field);
}
const restored = createPrivateKey({ key: Buffer.from(Fido2Utils.stringToArray(stored.keyValue)), format: "der", type: "pkcs8" });
const challenge = randomBytes(32);
if (!verify("sha256", challenge, publicKey, sign("sha256", challenge, restored))) throw new Error("Cle relue non utilisable");
bytes.fill(0);
console.log(JSON.stringify({ success: true, upstreamCliRoundTrip: true, fictitiousKeyOnly: true }));
