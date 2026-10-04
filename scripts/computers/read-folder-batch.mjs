// Each batch shares one SDK session and fetches at most four folders in parallel.
// Children are addressed by verified UID; no repeated ancestor/path traversal.
export async function readFolderBatch(sdk, paths, input) {
  const requests = JSON.parse(input);
  if (!Array.isArray(requests) || requests.length < 1 || requests.length > 4)
    throw new Error("Invalid metadata batch.");
  for (const request of requests) {
    if (
      !request ||
      typeof request.path !== "string" ||
      !/^\/my-files(?:\/[^/]+)*$/u.test(request.path) ||
      request.path.split("/").some((part) => [".", ".."].includes(part)) ||
      /[\\\x00-\x1f\x7f]/u.test(request.path) ||
      (request.uid !== null &&
        (typeof request.uid !== "string" ||
          !request.uid ||
          request.uid.length > 512))
    )
      throw new Error("Invalid metadata batch.");
  }
  return Promise.all(
    requests.map(async ({ path, uid }) => {
      const parent = uid ?? (await paths.getNode(path));
      const entries = [];
      for await (const node of sdk.iterateFolderChildren(parent)) {
        if (entries.length >= 100000)
          throw new Error(
            "Folder exceeds the limit of 100 levels or 100,000 items.",
          );
        entries.push(node);
      }
      return { path, entries };
    }),
  );
}
