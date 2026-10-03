import type { LibraryView, RemoteEntry } from "../app/types";
export const libraryView = (
  entries: Partial<RemoteEntry>[] = [],
  overrides: Partial<LibraryView> = {},
): LibraryView => ({
  entries: entries as RemoteEntry[],
  revision: 1,
  updatedAt: 1791000000,
  refreshing: false,
  partial: false,
  foldersDone: 1,
  foldersPending: 0,
  error: null,
  ...overrides,
});
