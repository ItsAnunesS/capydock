// Compiled alongside the unmodified, pinned official CLI sources. Authentication
// stays in the CLI's OS keyring and is never sent to the frontend.
import { run, applyDefaultCliOptions, printObject } from "./cli";
import { readFolderBatch } from "./read-folder-batch.mjs";
import { ensureDevice } from "./ensure-device.mjs";

declare const APP_VERSION: string;
declare const SDK_VERSION: string;

const commands = applyDefaultCliOptions([
  {
    group: "library",
    name: "list-batch",
    args: ["requests"],
    help: "Read up to four folders with one authenticated SDK session.",
    async action({ sdk, paths, args: [requests], options: { json } }: any) {
      printObject(await readFolderBatch(sdk, paths, requests), json);
    },
  },
  {
    group: "device",
    name: "ensure",
    args: ["name"],
    help: "Register this Linux computer, or reuse its unique existing name.",
    async action({ sdk, args: [name], options: { json } }: any) {
      printObject(await ensureDevice(sdk, name), json);
    },
  },
]);

try {
  await run(commands, {
    clientUidPrefix: "sdk-js-cli",
    appVersion: APP_VERSION,
    sdkVersion: SDK_VERSION,
    enablePersistedEvents: false,
    enableMetrics: false,
  });
  process.exit(0);
} catch (error) {
  console.error(
    error instanceof Error ? error.message : "Falha ao registrar computador.",
  );
  process.exit(1);
}
