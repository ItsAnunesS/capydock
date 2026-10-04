// Device names form part of the official CLI's virtual path, so ambiguous names
// must never select another computer silently.
export async function ensureDevice(sdk, name) {
  if (
    !name ||
    name.length > 120 ||
    /[/\\\x00-\x1f\x7f]/u.test(name) ||
    [".", ".."].includes(name)
  )
    throw new Error("Choose a valid computer name, up to 120 characters.");
  const matches = [];
  for await (const device of sdk.iterateDevices()) {
    if (!device.name.ok) throw new Error("Couldn't verify the computer names.");
    if (device.name.value === name) matches.push(device);
  }
  if (matches.length > 1)
    throw new Error(
      "Computers have the same name. Rename them in Proton Drive.",
    );
  if (matches.length && matches[0].type !== "Linux")
    throw new Error(
      "That name already belongs to another computer. Choose a different name.",
    );
  return matches[0] ?? (await sdk.createDevice(name, "Linux"));
}
