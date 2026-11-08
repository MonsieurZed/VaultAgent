/* Adaptation GPL du point d'entrée Bitwarden : moteur persistant sur stdin/stdout, sans serveur réseau. */
import "core-js/proposals/explicit-resource-management";
import { createInterface } from "node:readline";
import { IncomingMessage, ServerResponse } from "node:http";
import { Socket } from "node:net";
import { Router } from "@koa/router";
import * as Koa from "koa";
import { ServiceContainer } from "@agent-upstream/service-container/service-container";
import { OssServeConfigurator } from "@agent-upstream/oss-serve-configurator";
import { LoginCommand } from "@agent-upstream/auth/commands/login.command";
import { ConfigCommand } from "@agent-upstream/platform/commands/config.command";
import { firstValueFrom } from "rxjs";
import { getUserId } from "@bitwarden/common/auth/services/account.service";
import { CipherView } from "@bitwarden/common/vault/models/view/cipher.view";
import { PolicyType } from "@bitwarden/common/admin-console/enums";
import { applyWritePatch } from "./homelab-write-patch.mjs";

const write = (value: unknown) => process.stdout.write(JSON.stringify(value) + "\n");
console.log = console.info = console.warn = console.error = () => {};
process.env.BW_NOINTERACTION = "true";
process.env.BW_SERVE = "true";

function category(error: unknown): string {
  const value = error as { message?: unknown; code?: unknown; name?: unknown };
  const message = typeof value?.message === "string" ? value.message : "";
  if (/two.step|two.factor|code is required|verification code/i.test(message)) return "two-factor";
  if (/invalid master password|username or password is incorrect|invalid credentials/i.test(message)) return "credentials";
  if (/captcha/i.test(message)) return "captcha";
  if (/certificate|self.signed|issuer|tls/i.test(message)) return "tls";
  if (/fetch failed|ENOTFOUND|ECONNREFUSED|ETIMEDOUT|network/i.test(message)) return "network";
  if (/not a function|not defined|undefined|null/i.test(message)) return "engine-api";
  return value?.name === "TypeError" ? "engine-type" : "rejected";
}

async function main() {
  const c = new ServiceContainer();
  await c.init();
  const configurator = new OssServeConfigurator(c);
  const router = new Router();
  await configurator.configureRouter(router);
  const route = router.routes();
  const app = new Koa();
  const login = new LoginCommand(c.loginStrategyService, c.authService, c.twoFactorApiService,
    c.cryptoFunctionService, c.environmentService, c.passwordGenerationService, c.platformUtilsService,
    c.accountService, c.twoFactorService, c.syncService, c.keyConnectorService, async () => await c.logout(),
    c.ssoUrlService, c.i18nService, c.masterPasswordService, c.userDecryptionOptionsService, c.encryptedMigrator);
  async function call(method: string, path: string, body?: unknown) {
    if (!/^(GET|POST|PUT|DELETE)$/.test(method) || !path.startsWith("/")) throw new Error("request");
    const req = new IncomingMessage(new Socket());
    req.method = method;
    req.url = path;
    req.headers = { host: "localhost" };
    const res = new ServerResponse(req);
    const ctx = app.createContext(req, res);
    ctx.request.body = body;
    await route(ctx, async () => {});
    const result = ctx.body as { success: boolean; data?: any };
    if (!result?.success) throw new Error("operation");
    return result.data?.object === "template" ? result.data.template : result.data;
  }
  write({ ready: true, version: "2026.10.1-agent.2" });
  try {
    for await (const line of createInterface({ input: process.stdin, crlfDelay: Infinity })) {
      let id: unknown;
      let stage = "request";
      try {
        if (line.length > 4 * 1024 * 1024) throw new Error("size");
        const request = JSON.parse(line);
        id = request.id;
        let data: unknown;
        if (request.op === "prepare") {
          stage = "config";
          const state = await call("GET", "/status") as { status: string; serverUrl: string; userEmail: string };
          const configuredServer = state.serverUrl || "https://bitwarden.com";
          if (state.status !== "unauthenticated" &&
              (configuredServer !== request.server || state.userEmail?.toLowerCase() !== request.email.toLowerCase())) throw new Error("account");
          const restartRequired = state.status === "unauthenticated" && configuredServer !== request.server;
          if (restartRequired) {
            const configured = await new ConfigCommand(c.environmentService, c.accountService).run("server", request.server, {});
            if (!configured.success) throw configured;
          }
          data = { restartRequired };
        } else if (request.op === "connect") {
          stage = "status";
          const state = await call("GET", "/status") as { status: string; serverUrl: string; userEmail: string };
          if (state.status !== "unauthenticated" &&
              ((state.serverUrl || "https://bitwarden.com") !== request.server || state.userEmail?.toLowerCase() !== request.email.toLowerCase())) {
            throw new Error("account");
          }
          if (state.status === "unauthenticated") {
            if ((state.serverUrl || "https://bitwarden.com") !== request.server) throw new Error("prepare-required");
            stage = "login";
            const result = await login.run(request.email, request.password,
              request.code ? { method: "0", code: request.code } : {});
            if (!result.success) throw result;
          } else if (state.status !== "unlocked") {
            stage = "unlock";
            await call("POST", "/unlock", { password: request.password });
          }
          data = { unlocked: true };
        } else if (request.op === "passkey-list") {
          stage = "passkeys";
          const result = await call("GET", "/list/object/items");
          data = result.data.flatMap((item: any) => (item.login?.fido2Credentials || [])
            .filter((credential: any) => !request.rpId || credential.rpId === request.rpId)
            .map((credential: any) => ({ itemId: item.id, credential: { ...credential,
              counter: String(credential.counter ?? "0"), discoverable: String(credential.discoverable ?? "false") } })));
        } else if (request.op === "passkey-create") {
          stage = "passkeys";
          const credential = request.credential;
          const item = await call("POST", "/object/item", { type: 1,
            name: `${credential.rpName || credential.rpId} — ${credential.userName || "Passkey"}`,
            login: { username: credential.userName, password: null, totp: null,
              uris: [{ uri: "https://" + credential.rpId, match: null }], fido2Credentials: [credential] } });
          const saved = await call("GET", "/object/item/" + item.id);
          const value = saved.login?.fido2Credentials?.find((entry: any) => entry.credentialId === credential.credentialId);
          if (!value || value.keyValue !== credential.keyValue || value.rpId !== credential.rpId) throw new Error("passkey-storage");
          data = { id: item.id, verified: true };
        } else if (request.op === "safe-update") {
          stage = "write";
          const userId = await firstValueFrom(c.accountService.activeAccount$.pipe(getUserId));
          const cipher = await c.cipherService.get(request.itemId, userId);
          if (!cipher || !await firstValueFrom(c.cipherAuthorizationService.canEditCipher$(cipher))) throw new Error("refused");
          if (await firstValueFrom(c.policyService.policyAppliesToUser$(PolicyType.OrganizationDataOwnership, userId))) throw new Error("refused");
          const original = await c.cipherService.decrypt(cipher, userId);
          const view = CipherView.fromJSON(JSON.parse(JSON.stringify(original)))!;
          applyWritePatch(view, request.expectedRevisionDate, request.patch);
          await c.cipherService.updateWithServer(view, userId, original);
          data = { id: request.itemId };
        } else if (request.op === "call") {
          if (request.path.startsWith("/unlock")) throw new Error("reserved");
          data = await call(request.method, request.path, request.body);
        } else if (request.op === "stop") {
          try { await call("POST", "/lock"); } catch {}
          write({ id, success: true });
          break;
        } else throw new Error("operation");
        write({ id, success: true, data });
      } catch (error) {
        write({ id, success: false, error: "Bitwarden operation failed", code: `${stage}:${category(error)}` });
      }
    }
  } finally { c.dispose(); }
}
main().catch(() => { write({ ready: false, error: "Engine initialization failed" }); process.exitCode = 1; });
