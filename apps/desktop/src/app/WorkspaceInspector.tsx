import type { WorkspaceController } from "../application/workspace/controller.js";
import { projectHasActiveTurn } from "../domain/sessions/workspaceState.js";
import { ProjectInspector } from "../ui/files/ProjectInspector.js";
import type { WorkspaceFocus } from "./useWorkspaceFocus.js";
import type { WorkspaceSurfaces } from "./useWorkspaceSurfaces.js";

export function WorkspaceInspector({
  app,
  surfaces,
  focus,
}: {
  app: WorkspaceController;
  surfaces: WorkspaceSurfaces;
  focus: WorkspaceFocus;
}) {
  if (!app.project) return null;
  return (
    <ProjectInspector
      key={app.project.path}
      project={app.project}
      mode={surfaces.pane}
      changeMode={surfaces.changePane}
      expanded={surfaces.expanded}
      setExpanded={surfaces.setExpanded}
      request={surfaces.fileRequest}
      canAttach={
        !app.selected?.isSubmitting &&
        !(!app.selectedId && app.providerWorkspace?.isCreating)
      }
      attach={(file, ask) => {
        if (!surfaces.isCurrentSession()) return;
        app.addFileContext(file, ask);
        surfaces.setExpanded(false);
        requestAnimationFrame(() => {
          if (surfaces.isCurrentSession()) focus.textareaRef.current?.focus();
        });
      }}
      working={Boolean(app.workspace && projectHasActiveTurn(app.workspace))}
      close={() => {
        surfaces.closeInspector();
        focus.request("composer");
      }}
    />
  );
}
