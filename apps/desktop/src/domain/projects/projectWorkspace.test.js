import assert from "node:assert/strict";
import test from "node:test";

import { addOpenProject, closeOpenProject, selectOpenProject } from "./projectWorkspace.js";

const projectA = { name: "alpha", path: "/work/alpha" };
const projectB = { name: "beta", path: "/work/beta" };

test("adding a project keeps existing projects and avoids duplicate canonical paths", () => {
  const projects = [projectA];

  assert.deepEqual(addOpenProject(projects, projectB), [projectA, projectB]);
  assert.equal(addOpenProject(projects, { ...projectA }).length, 1);
  assert.equal(addOpenProject(projects, projectA), projects);
});

test("selecting an open project resolves it by canonical path", () => {
  assert.equal(selectOpenProject([projectA, projectB], projectB.path), projectB);
  assert.equal(selectOpenProject([projectA, projectB], "/work/missing"), null);
});

test("closing an inactive project keeps the active project selected", () => {
  const result = closeOpenProject([projectA, projectB], projectA.path, projectB.path);

  assert.deepEqual(result.projects, [projectA]);
  assert.equal(result.activeProject, projectA);
});

test("closing the active project selects its next neighbor or the previous last project", () => {
  const projects = [projectA, projectB, { name: "gamma", path: "/work/gamma" }];
  const middle = closeOpenProject(projects, projectB.path, projectB.path);
  const last = closeOpenProject(projects, "/work/gamma", "/work/gamma");

  assert.equal(middle.activeProject?.path, "/work/gamma");
  assert.equal(last.activeProject, projectB);
});

test("closing the last project leaves no active project", () => {
  const result = closeOpenProject([projectA], projectA.path, projectA.path);

  assert.deepEqual(result.projects, []);
  assert.equal(result.activeProject, null);
});
