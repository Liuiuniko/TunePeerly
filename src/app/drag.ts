// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import type { PointerEvent as ReactPointerEvent } from "react";
import type { Track } from "../api";

export type DragSource =
  | { type: "tracks"; tracks: Track[] }
  | { type: "queue"; index: number; tracks: Track[] }
  | { type: "member"; playlistId: string; index: number; tracks: Track[] };

/** Parsed from `data-drop` attributes, e.g. "playlist:ID", "liked", "queue:3", "member:ID:2". */
export type DropTarget =
  | { type: "playlist"; id: string }
  | { type: "liked" }
  | { type: "queue"; index: number }
  | { type: "member"; playlistId: string; index: number };

type DropHandler = (source: DragSource, target: DropTarget) => void;

let handler: DropHandler = () => {};
let suppressClick = false;

export function setDropHandler(next: DropHandler): void {
  handler = next;
}

function parseTarget(element: HTMLElement, clientY: number): DropTarget | null {
  const value = element.dataset.drop ?? "";
  const [kind, first, second] = value.split(":");
  const rect = element.getBoundingClientRect();
  const after = clientY > rect.top + rect.height / 2;
  if (kind === "playlist" && first) return { type: "playlist", id: first };
  if (kind === "liked") return { type: "liked" };
  if (kind === "queue") return { type: "queue", index: Number(first) + (after ? 1 : 0) };
  if (kind === "member" && first) return { type: "member", playlistId: first, index: Number(second) + (after ? 1 : 0) };
  return null;
}

function accepts(source: DragSource, target: DropTarget): boolean {
  if (target.type === "queue") return source.type === "queue" || source.type === "tracks";
  if (target.type === "member") return source.type === "member" && source.playlistId === target.playlistId;
  return true;
}

/** Starts a pointer drag after a short move; a plain click still goes through. */
export function beginDrag(event: ReactPointerEvent, source: () => DragSource | null, label: () => string): void {
  if (event.button !== 0 || (event.target as HTMLElement).closest("button, input, a, select, textarea")) return;
  const startX = event.clientX;
  const startY = event.clientY;
  let ghost: HTMLDivElement | null = null;
  let payload: DragSource | null = null;
  let over: HTMLElement | null = null;
  let target: DropTarget | null = null;

  const clear = () => {
    over?.classList.remove("drop-over", "drop-before", "drop-after");
    over = null;
  };

  const move = (moveEvent: PointerEvent) => {
    if (!ghost) {
      if (Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) < 6) return;
      payload = source();
      if (!payload) return finish();
      ghost = document.createElement("div");
      ghost.className = "drag-ghost";
      ghost.textContent = label();
      document.body.appendChild(ghost);
      document.body.classList.add("dragging");
    }
    ghost.style.transform = `translate(${moveEvent.clientX + 14}px, ${moveEvent.clientY + 10}px)`;
    const found = (document.elementFromPoint(moveEvent.clientX, moveEvent.clientY) as HTMLElement | null)?.closest<HTMLElement>("[data-drop]") ?? null;
    const parsed = found && payload ? parseTarget(found, moveEvent.clientY) : null;
    target = parsed && payload && accepts(payload, parsed) ? parsed : null;
    if (found !== over) clear();
    if (found && target) {
      over = found;
      found.classList.add("drop-over");
      if (target.type === "queue" || target.type === "member") {
        const rect = found.getBoundingClientRect();
        const after = moveEvent.clientY > rect.top + rect.height / 2;
        found.classList.toggle("drop-after", after);
        found.classList.toggle("drop-before", !after);
      }
    }
  };

  const finish = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", cancel);
    window.removeEventListener("blur", finish);
    clear();
    ghost?.remove();
    document.body.classList.remove("dragging");
  };

  const up = () => {
    const dropped = ghost !== null;
    const chosen = target;
    const carried = payload;
    finish();
    if (dropped) {
      suppressClick = true;
      window.setTimeout(() => (suppressClick = false), 0);
      if (chosen && carried) handler(carried, chosen);
    }
  };

  const cancel = () => finish();

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", cancel);
  window.addEventListener("blur", finish);
}

/** True for the click event that ends a drag. */
export function clickWasDrag(): boolean {
  return suppressClick;
}
