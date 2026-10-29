export function addOpenProject(projects, project) {
  if (projects.some((openProject) => openProject.path === project.path)) return projects;
  return [...projects, project];
}

export function selectOpenProject(projects, path) {
  return projects.find((project) => project.path === path) ?? null;
}

export function closeOpenProject(projects, activePath, closingPath) {
  const closingIndex = projects.findIndex((project) => project.path === closingPath);
  if (closingIndex < 0) {
    return {
      projects,
      activeProject: selectOpenProject(projects, activePath),
    };
  }

  const remaining = projects.filter((project) => project.path !== closingPath);
  if (activePath !== closingPath) {
    return {
      projects: remaining,
      activeProject: selectOpenProject(remaining, activePath),
    };
  }

  const nextIndex = Math.min(closingIndex, remaining.length - 1);
  return {
    projects: remaining,
    activeProject: remaining[nextIndex] ?? null,
  };
}
