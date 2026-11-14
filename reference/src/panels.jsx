// The react-resizable-panels reference page: the scenario named by the URL
// hash from tests/fixtures/fidelity/panels-scenarios.json, rendered with the
// package as a React app renders it. tools/fidelity/capture.mjs drives it in
// pinned Chromium and reads window.__panels.snapshot() after each step.

import { createRoot } from "react-dom/client";
import { Group, Panel, Separator } from "react-resizable-panels";
import spec from "../../tests/fixtures/fidelity/panels-scenarios.json";

const scenario = spec.scenarios.find((entry) => `#${entry.id}` === location.hash);
const changes = [];
const changed = [];
const clicks = [];

function panelIds(group) {
  return group.children.filter((child) => child.panel).map((child) => child.panel.id);
}

// The package reports a layout as { [panelId]: percent }; the snapshot lists
// it in panel order, as the port reports it.
function inOrder(group, layout) {
  return panelIds(group).map((id) => layout[id]);
}

function GroupView({ group }) {
  const separatorStyle =
    group.orientation === "horizontal"
      ? { width: `${spec.separatorSize}px` }
      : { height: `${spec.separatorSize}px` };
  return (
    <Group
      id={group.id}
      orientation={group.orientation}
      onLayoutChange={(layout) => changes.push({ group: group.id, layout: inOrder(group, layout) })}
      onLayoutChanged={(layout) => changed.push({ group: group.id, layout: inOrder(group, layout) })}
    >
      {group.children.map((child) => {
        if (child.separator) {
          const { id, disabled } = child.separator;
          return <Separator key={id} id={id} disabled={disabled} style={separatorStyle} />;
        }
        const { id, group: nested, ...sizes } = child.panel;
        return (
          <Panel key={id} id={id} {...sizes}>
            {nested ? <GroupView group={nested} /> : null}
          </Panel>
        );
      })}
    </Group>
  );
}

document.addEventListener("click", (event) => clicks.push(event.target.id || event.target.localName), true);

function snapshot() {
  const elements = [];
  for (const element of document.querySelectorAll("[data-group], [data-panel], [data-separator]")) {
    const rect = element.getBoundingClientRect();
    const attributes = {};
    for (const { name, value } of element.attributes) {
      if (name !== "style" && name !== "class") {
        attributes[name] = value;
      }
    }
    const kind = element.hasAttribute("data-group")
      ? "group"
      : element.hasAttribute("data-panel")
        ? "panel"
        : "separator";
    elements.push({ id: element.id, kind, rect: [rect.x, rect.y, rect.width, rect.height], attributes });
  }
  const active = document.activeElement;
  return {
    elements,
    focus: active && active !== document.body ? active.id : null,
    cursor: getComputedStyle(document.documentElement).cursor,
    changes: changes.splice(0),
    changed: changed.splice(0),
    clicks: clicks.splice(0),
  };
}

// Two frames and a task: React commits, the package's effects run, and its
// deferred work (the click suppression reset) finishes.
function settle() {
  return new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 0))),
  );
}

window.__panels = { snapshot, settle, scenario: scenario?.id ?? null };
createRoot(document.getElementById("host")).render(scenario ? <GroupView group={scenario.group} /> : null);
