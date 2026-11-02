export type ProjectInfo = { name: string; path: string };

export function addOpenProject<T extends ProjectInfo>(projects: T[], project: T): T[];

export function selectOpenProject<T extends ProjectInfo>(projects: T[], path: string | null): T | null;

export function closeOpenProject<T extends ProjectInfo>(
  projects: T[],
  activePath: string | null,
  closingPath: string,
): { projects: T[]; activeProject: T | null };
