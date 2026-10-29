<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from "svelte";
  import {
    forceCenter,
    forceCollide,
    forceLink,
    forceManyBody,
    forceSimulation,
    type Simulation,
    type SimulationNodeDatum,
  } from "d3-force";
  import { nameOf, type NoteInfo } from "./api";
  import { linkTarget } from "./links";
  import IconButton from "./IconButton.svelte";
  import { Close } from "./icons";

  export let notes: string[];
  export let index: NoteInfo[];
  export let current: string | null;

  type Node = SimulationNodeDatum & { id: string; degree: number };
  type Edge = { source: Node; target: Node };

  const dispatch = createEventDispatcher<{ open: string; close: void }>();
  let canvas: HTMLCanvasElement;
  let wrap: HTMLElement;
  let sim: Simulation<Node, Edge>;
  let nodes: Node[] = [];
  let edges: Edge[] = [];
  let neighbors = new Map<string, Set<string>>();
  let hover: Node | null = null;
  // View transform: screen = world * k + (x, y).
  let view = { x: 0, y: 0, k: 1 };
  let drag: { node: Node | null; startX: number; startY: number; moved: boolean } | null = null;

  function build() {
    const byId = new Map(notes.map((id) => [id, { id, degree: 0 } as Node]));
    const seen = new Set<string>();
    const list: Edge[] = [];
    for (const note of index) {
      const from = byId.get(note.path);
      if (!from) continue;
      for (const l of note.links) {
        const to = byId.get(linkTarget(l, note.path, notes) ?? "");
        const key = [note.path, to?.id].sort().join("\n");
        if (!to || to === from || seen.has(key)) continue;
        seen.add(key);
        list.push({ source: from, target: to });
        from.degree++;
        to.degree++;
      }
    }
    nodes = [...byId.values()];
    edges = list;
    for (const e of edges) {
      if (!neighbors.has(e.source.id)) neighbors.set(e.source.id, new Set());
      if (!neighbors.has(e.target.id)) neighbors.set(e.target.id, new Set());
      neighbors.get(e.source.id)!.add(e.target.id);
      neighbors.get(e.target.id)!.add(e.source.id);
    }
  }

  const radius = (n: Node) => 4 + Math.min(8, Math.sqrt(n.degree) * 2);

  function draw() {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const dpr = devicePixelRatio || 1;
    const { width, height } = canvas;
    const css = getComputedStyle(wrap);
    const color = (v: string) => css.getPropertyValue(v).trim();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.setTransform(dpr * view.k, 0, 0, dpr * view.k, dpr * view.x, dpr * view.y);

    const focus = hover ?? nodes.find((n) => n.id === current) ?? null;
    const near = focus ? neighbors.get(focus.id) ?? new Set<string>() : null;
    const lit = (n: Node) => !focus || n === focus || near!.has(n.id);

    ctx.lineWidth = 1 / view.k;
    for (const e of edges) {
      const on = focus && (e.source === focus || e.target === focus);
      ctx.strokeStyle = on ? color("--accent") : color("--border");
      ctx.globalAlpha = focus && !on ? 0.35 : 1;
      ctx.beginPath();
      ctx.moveTo(e.source.x!, e.source.y!);
      ctx.lineTo(e.target.x!, e.target.y!);
      ctx.stroke();
    }

    const showAll = nodes.length <= 80 || view.k > 1.6;
    ctx.font = `${12 / view.k}px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (const n of nodes) {
      ctx.globalAlpha = lit(n) ? 1 : 0.3;
      ctx.fillStyle = n.id === current ? color("--accent") : n === hover ? color("--text") : color("--muted");
      ctx.beginPath();
      ctx.arc(n.x!, n.y!, radius(n), 0, Math.PI * 2);
      ctx.fill();
      if (showAll || (focus && lit(n))) {
        ctx.fillStyle = color("--text");
        ctx.fillText(nameOf(n.id), n.x!, n.y! + radius(n) + 3 / view.k);
      }
    }
    ctx.globalAlpha = 1;
  }

  function resize() {
    const dpr = devicePixelRatio || 1;
    const rect = wrap.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;
    draw();
  }

  /** The node under a screen point, if any. */
  function nodeAt(clientX: number, clientY: number) {
    const rect = canvas.getBoundingClientRect();
    const x = (clientX - rect.left - view.x) / view.k;
    const y = (clientY - rect.top - view.y) / view.k;
    let best: Node | null = null;
    let bestD = Infinity;
    for (const n of nodes) {
      const d = Math.hypot(n.x! - x, n.y! - y);
      if (d < radius(n) + 4 / view.k && d < bestD) [best, bestD] = [n, d];
    }
    return best;
  }

  function onDown(e: PointerEvent) {
    canvas.setPointerCapture(e.pointerId);
    drag = { node: nodeAt(e.clientX, e.clientY), startX: e.clientX, startY: e.clientY, moved: false };
    if (drag.node) sim.alphaTarget(0.2).restart();
  }

  function onMove(e: PointerEvent) {
    if (!drag) {
      const h = nodeAt(e.clientX, e.clientY);
      if (h !== hover) {
        hover = h;
        canvas.style.cursor = h ? "pointer" : "grab";
        draw();
      }
      return;
    }
    if (Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) > 3) drag.moved = true;
    if (drag.node) {
      const rect = canvas.getBoundingClientRect();
      drag.node.fx = (e.clientX - rect.left - view.x) / view.k;
      drag.node.fy = (e.clientY - rect.top - view.y) / view.k;
    } else {
      view = { ...view, x: view.x + e.movementX, y: view.y + e.movementY };
      draw();
    }
  }

  function onUp() {
    if (!drag) return;
    const { node, moved } = drag;
    drag = null;
    if (node) {
      node.fx = node.fy = null;
      sim.alphaTarget(0);
      if (!moved) dispatch("open", node.id);
    }
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    const k = Math.max(0.2, Math.min(4, view.k * Math.exp(-e.deltaY * 0.002)));
    view = { k, x: px - ((px - view.x) / view.k) * k, y: py - ((py - view.y) / view.k) * k };
    draw();
  }

  let observer: ResizeObserver;

  onMount(() => {
    build();
    const rect = wrap.getBoundingClientRect();
    view = { x: rect.width / 2, y: rect.height / 2, k: 1 };
    sim = forceSimulation(nodes)
      .force("link", forceLink<Node, Edge>(edges).distance(70).strength(0.6))
      .force("charge", forceManyBody().strength(-160))
      .force("center", forceCenter(0, 0).strength(0.05))
      .force("collide", forceCollide<Node>().radius((n) => radius(n) + 6))
      .on("tick", draw);
    observer = new ResizeObserver(resize);
    observer.observe(wrap);
  });

  onDestroy(() => {
    sim?.stop();
    observer?.disconnect();
  });
</script>

<div class="graph" bind:this={wrap}>
  <div class="bar">
    <span>{nodes.length} notes · {edges.length} links</span>
    <span class="hint">Click a note to open it · drag to move · scroll to zoom</span>
    <span class="close"><IconButton icon={Close} label="Close graph (Esc)" on:click={() => dispatch("close")} /></span>
  </div>
  <canvas
    bind:this={canvas}
    on:pointerdown={onDown}
    on:pointermove={onMove}
    on:pointerup={onUp}
    on:pointercancel={onUp}
    on:wheel={onWheel}
  />
</div>

<style>
  .graph {
    position: absolute;
    inset: 0;
    background: var(--bg);
    z-index: 5;
  }
  canvas {
    display: block;
    cursor: grab;
    touch-action: none;
  }
  .bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
    font-size: 12px;
    color: var(--muted);
    pointer-events: none;
  }
  .hint {
    flex: 1;
  }
  .close {
    pointer-events: auto;
  }
</style>
