// Device names form part of the official CLI's virtual path, so ambiguous names
// must never select another computer silently.
export async function ensureDevice(sdk, name) {
  if (
    !name ||
    name.length > 120 ||
    /[/\\\x00-\x1f\x7f]/u.test(name) ||
    [".", ".."].includes(name)
  )
    throw new Error(
      "Escolha um nome de computador válido, com até 120 caracteres.",
    );
  const matches = [];
  for await (const device of sdk.iterateDevices()) {
    if (!device.name.ok)
      throw new Error("Não foi possível verificar os nomes dos computadores.");
    if (device.name.value === name) matches.push(device);
  }
  if (matches.length > 1)
    throw new Error(
      "Há computadores com o mesmo nome. Renomeie-os no Proton Drive.",
    );
  if (matches.length && matches[0].type !== "Linux")
    throw new Error(
      "Esse nome já pertence a outro computador. Escolha um nome diferente.",
    );
  return matches[0] ?? (await sdk.createDevice(name, "Linux"));
}
