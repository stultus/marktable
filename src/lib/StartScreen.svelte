<script lang="ts">
  import { onMount } from "svelte";
  import { animate, stagger } from "animejs";
  import {
    pickFolder,
    pickFile,
    openFolder,
    openFile,
    addRecent,
    getRecents,
    removeRecent,
    type TableModel,
    type RecentItem,
  } from "./api";

  let { onOpen }: { onOpen: (t: TableModel) => void } = $props();
  let busy = $state(false);
  let error = $state<string | null>(null);
  let busyKind = $state<"folder" | "file" | null>(null);
  let recents = $state<RecentItem[]>([]);
  let heroEl = $state<SVGSVGElement | null>(null);

  onMount(async () => {
    try {
      recents = await getRecents();
    } catch {
      // Recents store may not exist yet on first launch — ignore.
      recents = [];
    }

    // Skip the orchestrated reveal entirely if the user wants reduced motion.
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) {
      document.querySelectorAll<HTMLElement>(".reveal").forEach((el) => {
        el.style.opacity = "1";
        el.style.transform = "none";
      });
      return;
    }

    // The signature reveal: scattered "files" snap into a tidy table.
    if (heroEl) {
      // Phase 1 — outer card pops in
      animate(".hero-frame", {
        opacity: [0, 1],
        scale: [0.92, 1],
        duration: 480,
        ease: "out(3)",
      });
      // Phase 2 — header band slides in from the top
      animate(".hero-header-band", {
        opacity: [0, 1],
        translateY: [-6, 0],
        delay: 200,
        duration: 360,
        ease: "outQuart",
      });
      // Phase 3 — column type-glyph badges stagger in
      animate(".hero-glyph", {
        opacity: [0, 1],
        scale: [0.4, 1],
        delay: stagger(70, { start: 360 }),
        duration: 380,
        ease: "out(2)",
      });
      // Phase 4 — grid lines draw in (interior only — outer is the .hero-frame)
      animate(".hero-line", {
        opacity: [0, 1],
        scaleX: [0.2, 1],
        scaleY: [0.2, 1],
        delay: stagger(50, { start: 580 }),
        duration: 320,
        ease: "outQuart",
      });
      // Phase 5 — data cell bars stream in left-to-right
      animate(".hero-cell", {
        opacity: [0, 1],
        translateX: [-12, 0],
        delay: stagger(60, { start: 780 }),
        duration: 340,
        ease: "outQuart",
      });
      // Phase 6 — accent highlight cell pulses in
      animate(".hero-highlight", {
        opacity: [0, 0.55, 0.35],
        scale: [0.7, 1.04, 1],
        delay: 1180,
        duration: 720,
        ease: "outQuart",
      });
      // The text caret: fade in once, then loop a square-wave blink forever
      // (real text cursors snap on/off rather than fading, so duration:0 on
      // each transition is intentional).
      animate(".hero-cursor", {
        opacity: [0, 1],
        delay: 1500,
        duration: 200,
        ease: "linear",
        onComplete: () => {
          animate(".hero-cursor", {
            keyframes: [
              { opacity: 1, duration: 530 },
              { opacity: 0, duration: 530 },
            ],
            loop: true,
          });
        },
      });
    }

    // Title + cards fade up after the hero finishes drawing.
    animate(".reveal", {
      opacity: [0, 1],
      translateY: [10, 0],
      delay: stagger(80, { start: 1500 }),
      duration: 500,
      ease: "outQuart",
    });
  });

  function basename(path: string): string {
    const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return i >= 0 ? path.slice(i + 1) : path;
  }
  function dirname(path: string): string {
    const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return i >= 0 ? path.slice(0, i) : "";
  }

  async function chooseFolder() {
    error = null;
    const path = await pickFolder();
    if (!path) return;
    await openFolderPath(path);
  }

  async function chooseFile() {
    error = null;
    const path = await pickFile();
    if (!path) return;
    await openFilePath(path);
  }

  async function openFolderPath(path: string) {
    busy = true;
    busyKind = "folder";
    try {
      const table = await openFolder(path);
      try { await addRecent(path, "folder"); } catch {}
      onOpen(table);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
      busyKind = null;
    }
  }

  async function openFilePath(path: string) {
    busy = true;
    busyKind = "file";
    try {
      const table = await openFile(path);
      const kind = path.toLowerCase().endsWith(".json") ? "json_file" : "yaml_file";
      try { await addRecent(path, kind); } catch {}
      onOpen(table);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
      busyKind = null;
    }
  }

  async function openRecent(item: RecentItem) {
    if (!item.exists) {
      error = `${basename(item.path)} no longer exists at this path.`;
      return;
    }
    error = null;
    if (item.kind === "folder") await openFolderPath(item.path);
    else await openFilePath(item.path);
  }

  async function dropRecent(path: string) {
    try {
      await removeRecent(path);
      recents = recents.filter((r) => r.path !== path);
    } catch {}
  }
</script>

<main class="start" data-busy={busy}>
  <header class="topbar">
    <div class="brand">
      <span class="brand-mark" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="16" height="16" fill="none">
          <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" stroke-width="1.4" />
          <path d="M5 2.5v11M11 2.5v11M1.5 6h13M1.5 10h13" stroke="currentColor" stroke-width="1.4" />
        </svg>
      </span>
      <span class="brand-name">MarkTable</span>
    </div>
  </header>

  <section class="hero">
    <!-- Banner-aspect mini table (≈7:1). Renders ~110px tall at the full
         content width, so it sits as a header strip rather than a hero
         block that eats the viewport. -->
    <svg
      bind:this={heroEl}
      class="hero-svg"
      viewBox="0 0 700 100"
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label="Animated illustration of a table strip"
    >
      <!-- Outer card -->
      <rect
        class="hero-frame"
        x="8"
        y="8"
        width="684"
        height="84"
        rx="8"
        fill="var(--mt-elevated)"
        stroke="var(--mt-border-strong)"
        stroke-width="1.2"
      />
      <!-- Header band -->
      <path
        class="hero-header-band"
        d="M 16 8 H 684 A 8 8 0 0 1 692 16 V 36 H 8 V 16 A 8 8 0 0 1 16 8 Z"
        fill="var(--mt-surface)"
      />
      <!-- Header divider -->
      <line class="hero-line" x1="8" y1="36" x2="692" y2="36"
            stroke="var(--mt-border-strong)" stroke-width="1.2"/>
      <!-- Vertical column dividers — 5 columns -->
      <line class="hero-line" x1="144" y1="8" x2="144" y2="92"
            stroke="var(--mt-divider)" stroke-width="1"/>
      <line class="hero-line" x1="280" y1="8" x2="280" y2="92"
            stroke="var(--mt-divider)" stroke-width="1"/>
      <line class="hero-line" x1="416" y1="8" x2="416" y2="92"
            stroke="var(--mt-divider)" stroke-width="1"/>
      <line class="hero-line" x1="552" y1="8" x2="552" y2="92"
            stroke="var(--mt-divider)" stroke-width="1"/>
      <!-- Type-glyph badges -->
      <rect class="hero-glyph" x="20" y="16" width="16" height="14" rx="3"
            fill="var(--mt-tag-blue)"/>
      <rect class="hero-glyph" x="156" y="16" width="16" height="14" rx="3"
            fill="var(--mt-tag-green)"/>
      <rect class="hero-glyph" x="292" y="16" width="16" height="14" rx="3"
            fill="var(--mt-tag-purple)"/>
      <rect class="hero-glyph" x="428" y="16" width="16" height="14" rx="3"
            fill="var(--mt-tag-orange)"/>
      <rect class="hero-glyph" x="564" y="16" width="16" height="14" rx="3"
            fill="var(--mt-tag-pink)"/>
      <!-- Highlighted cell (col 2) — the "selected cell" -->
      <rect
        class="hero-highlight"
        x="145"
        y="37"
        width="135"
        height="55"
        fill="var(--mt-accent-soft)"
        stroke="var(--mt-accent)"
        stroke-width="1.4"
      />
      <!-- Single row of content bars — keeps the strip readable at banner aspect -->
      <rect class="hero-cell" x="20" y="61" width="92" height="6" rx="3"
            fill="var(--mt-fg-muted)"/>
      <rect class="hero-cell" x="156" y="61" width="100" height="6" rx="3"
            fill="var(--mt-accent)"/>
      <rect class="hero-cell" x="292" y="61" width="86" height="6" rx="3"
            fill="var(--mt-fg-muted)"/>
      <rect class="hero-cell" x="428" y="61" width="78" height="6" rx="3"
            fill="var(--mt-fg-muted)"/>
      <rect class="hero-cell" x="564" y="61" width="100" height="6" rx="3"
            fill="var(--mt-fg-muted)"/>
      <!-- Editing cursor at the trailing edge of the highlighted cell's bar -->
      <line
        class="hero-cursor"
        x1="258"
        y1="56"
        x2="258"
        y2="74"
        stroke="var(--mt-accent)"
        stroke-width="1.4"
        stroke-linecap="round"
      />
    </svg>
    <h1 class="reveal">Edit structured content like a spreadsheet.</h1>
  </section>

  <section class="choices" aria-label="Open">
    <button
      type="button"
      class="choice reveal"
      onclick={chooseFolder}
      disabled={busy}
      aria-busy={busyKind === "folder"}
    >
      <span class="choice-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none">
          <path
            d="M3 7.2c0-.83.67-1.5 1.5-1.5h4.21c.4 0 .78.16 1.06.44l1.5 1.5c.28.28.66.44 1.06.44H19.5c.83 0 1.5.67 1.5 1.5v8.7c0 .83-.67 1.5-1.5 1.5h-15a1.5 1.5 0 01-1.5-1.5V7.2z"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linejoin="round"
          />
        </svg>
      </span>
      <span class="choice-body">
        <span class="choice-title">Open a folder</span>
        <span class="choice-desc">
          One <code>.md</code> file per row. Frontmatter fields become columns.
        </span>
        <span class="choice-meta">.md &nbsp;·&nbsp; YAML frontmatter</span>
      </span>
      <span class="choice-arrow" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
          <path d="M5 3l5 5-5 5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
    </button>

    <button
      type="button"
      class="choice reveal"
      onclick={chooseFile}
      disabled={busy}
      aria-busy={busyKind === "file"}
    >
      <span class="choice-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none">
          <path
            d="M14 3.2H6.5A1.5 1.5 0 005 4.7v14.6c0 .83.67 1.5 1.5 1.5h11a1.5 1.5 0 001.5-1.5V8.2L14 3.2z"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linejoin="round"
          />
          <path d="M14 3.2v5h5" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
          <path d="M8 12h8M8 15.5h8M8 18.5h5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        </svg>
      </span>
      <span class="choice-body">
        <span class="choice-title">Open a single file</span>
        <span class="choice-desc">
          A top-level array of objects, or a YAML list of mappings. One item
          per row.
        </span>
        <span class="choice-meta">.json &nbsp;·&nbsp; .yaml &nbsp;·&nbsp; .yml</span>
      </span>
      <span class="choice-arrow" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
          <path d="M5 3l5 5-5 5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
    </button>
  </section>

  {#if recents.length > 0}
    <section class="recents" aria-label="Recent">
      <header class="recents-head">
        <span class="recents-label">Recent</span>
      </header>
      <ul class="recents-list">
        {#each recents as item (item.path)}
          <li
            class="recent"
            class:missing={!item.exists}
          >
            <button
              type="button"
              class="recent-main"
              disabled={busy}
              onclick={() => openRecent(item)}
              title={item.path}
            >
              <span class="recent-icon" aria-hidden="true">
                {#if item.kind === "folder"}
                  <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
                    <path
                      d="M1.5 4.4c0-.5.4-.9.9-.9h2.95c.24 0 .47.1.64.27L7.1 4.85c.17.17.4.27.64.27H13.6c.5 0 .9.4.9.9v6.18c0 .5-.4.9-.9.9H2.4a.9.9 0 01-.9-.9V4.4z"
                      stroke="currentColor"
                      stroke-width="1.4"
                      stroke-linejoin="round"
                    />
                  </svg>
                {:else}
                  <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
                    <path
                      d="M9 1.8H4.2a.9.9 0 00-.9.9v10.6c0 .5.4.9.9.9h7.6a.9.9 0 00.9-.9V5.4L9 1.8z"
                      stroke="currentColor"
                      stroke-width="1.4"
                      stroke-linejoin="round"
                    />
                    <path d="M9 1.8v3.6h3.6" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
                  </svg>
                {/if}
              </span>
              <span class="recent-body">
                <span class="recent-name">{basename(item.path)}{#if !item.exists}<span class="recent-missing">— missing</span>{/if}</span>
                <span class="recent-dir">{dirname(item.path)}</span>
              </span>
              <span class="recent-kind">{item.kind === "folder" ? "folder" : item.kind === "json_file" ? ".json" : ".yaml"}</span>
            </button>
            <button
              type="button"
              class="recent-remove"
              onclick={() => dropRecent(item.path)}
              aria-label="Remove from recent"
              title="Remove from recent"
            >
              <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
                <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              </svg>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if error}
    <div class="error" role="alert">
      <span class="error-dot" aria-hidden="true"></span>
      <span class="error-text">{error}</span>
      <button class="error-close" onclick={() => (error = null)} aria-label="Dismiss error">
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none">
          <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  {/if}

</main>

<style>
  .start {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto auto auto auto 1fr;
    max-width: 880px;
    margin: 0 auto;
    padding: 24px 40px 28px;
    position: relative;
  }
  .start::before {
    /* Soft warm-paper grain that disappears in dark mode */
    content: "";
    position: fixed;
    inset: 0;
    pointer-events: none;
    background:
      radial-gradient(
        1200px 600px at 80% -10%,
        rgba(35, 131, 226, 0.045),
        transparent 60%
      ),
      radial-gradient(
        900px 500px at -10% 110%,
        rgba(203, 145, 47, 0.04),
        transparent 60%
      );
    z-index: 0;
  }
  .start > * {
    position: relative;
    z-index: 1;
  }

  /* Top bar */
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 0 0;
    color: var(--mt-fg-muted);
    font-size: 12px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .brand-mark {
    color: var(--mt-fg);
    display: inline-flex;
  }
  .brand-name {
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    letter-spacing: 0.02em;
    color: var(--mt-fg);
    font-weight: 500;
  }

  /* Hero */
  .hero {
    padding: 24px 0 24px;
    /* Width matches the .start container minus its padding so the hero SVG
       can span edge-to-edge of the choices grid below. */
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 22px;
  }
  /* Animated mini-table — orchestrated entrance via anime.js. The SVG and
     each animatable child start at opacity:0 so anime.js can ramp them in.
     Falls back to opacity:1 if reduce-motion is set (handled in onMount).
     Width matches the .choices section below it so the two visual blocks
     line up edge-to-edge. */
  .hero-svg {
    display: block;
    width: 100%;
    height: auto;
    /* Hard ceiling: even on very wide windows the strip never grows past
       a comfortable banner height. The viewBox aspect (~7:1) means the
       width self-limits too. */
    max-height: 130px;
    filter: drop-shadow(0 6px 18px rgba(35, 131, 226, 0.06));
  }
  .hero-frame,
  .hero-header-band,
  .hero-line,
  .hero-glyph,
  .hero-cell,
  .hero-highlight,
  .hero-cursor {
    opacity: 0;
    transform-box: fill-box;
    transform-origin: center;
  }
  h1 {
    margin: 0;
    font-family: var(--mt-font-display);
    font-weight: 500;
    font-size: clamp(20px, 2.2vw, 24px);
    line-height: 1.3;
    letter-spacing: -0.012em;
    color: var(--mt-fg);
  }
  /* Initial state for anime.js .reveal entries — invisible until the
     onMount sequence runs them. */
  .reveal {
    opacity: 0;
  }

  /* Choice cards */
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
    margin-top: 4px;
  }
  .choice {
    all: unset;
    cursor: pointer;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: start;
    gap: 16px;
    padding: 18px 20px 18px 18px;
    background: var(--mt-surface);
    border: 1px solid var(--mt-border);
    border-radius: 6px;
    color: var(--mt-fg);
    transition:
      background 140ms ease,
      border-color 140ms ease,
      transform 200ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
    position: relative;
  }
  .choice:hover:not(:disabled) {
    background: var(--mt-surface-strong);
    border-color: var(--mt-border-strong);
  }
  .choice:hover:not(:disabled) .choice-arrow {
    transform: translateX(3px);
    color: var(--mt-accent);
  }
  .choice:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 2px;
  }
  .choice:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .choice[aria-busy="true"]::after {
    content: "";
    position: absolute;
    top: 8px;
    right: 8px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px solid var(--mt-accent-soft);
    border-top-color: var(--mt-accent);
    animation: spin 720ms linear infinite;
  }
  .choice-icon {
    margin-top: 2px;
    color: var(--mt-fg-muted);
    flex-shrink: 0;
  }
  .choice:hover:not(:disabled) .choice-icon {
    color: var(--mt-fg);
  }
  .choice-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .choice-title {
    font-size: 14.5px;
    font-weight: 600;
    letter-spacing: -0.005em;
  }
  .choice-desc {
    font-size: 13px;
    color: var(--mt-fg-muted);
    line-height: 1.5;
  }
  .choice-desc code {
    font-family: var(--mt-font-mono);
    font-size: 12px;
    background: var(--mt-surface-strong);
    border: 1px solid var(--mt-divider);
    border-radius: 3px;
    padding: 0 4px;
    color: var(--mt-fg);
  }
  .choice-meta {
    margin-top: 6px;
    font-family: var(--mt-font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
    color: var(--mt-fg-subtle);
    text-transform: lowercase;
  }
  .choice-arrow {
    align-self: center;
    color: var(--mt-fg-subtle);
    transition:
      transform 200ms ease,
      color 200ms ease;
  }

  /* Recent items */
  .recents {
    margin-top: 28px;
    animation: rise 480ms cubic-bezier(0.2, 0.8, 0.3, 1) both;
    animation-delay: 320ms;
  }
  .recents-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 8px;
  }
  .recents-label {
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--mt-fg-subtle);
  }
  .recents-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--mt-divider);
    border: 1px solid var(--mt-border);
    border-radius: 6px;
    overflow: hidden;
  }
  .recent {
    display: flex;
    align-items: stretch;
    background: var(--mt-page-bg);
    transition: background 120ms ease;
  }
  .recent:hover {
    background: var(--mt-surface);
  }
  .recent-main {
    all: unset;
    flex: 1;
    cursor: pointer;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    color: var(--mt-fg);
    min-width: 0;
  }
  .recent-main:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: -2px;
    border-radius: 4px;
  }
  .recent-main:disabled {
    cursor: progress;
    opacity: 0.55;
  }
  .recent-icon {
    color: var(--mt-fg-subtle);
    display: inline-flex;
  }
  .recent:hover .recent-icon {
    color: var(--mt-fg-muted);
  }
  .recent-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .recent-name {
    font-size: 13.5px;
    font-weight: 500;
    color: var(--mt-fg);
    letter-spacing: -0.005em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .recent-missing {
    margin-left: 8px;
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mt-error);
  }
  .recent-dir {
    font-family: var(--mt-font-mono);
    font-size: 11px;
    color: var(--mt-fg-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .recent-kind {
    font-family: var(--mt-font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mt-fg-subtle);
    padding: 2px 7px;
    border: 1px solid var(--mt-divider);
    border-radius: 999px;
  }
  .recent.missing .recent-name,
  .recent.missing .recent-dir,
  .recent.missing .recent-icon {
    color: var(--mt-fg-subtle);
  }
  .recent.missing {
    opacity: 0.78;
  }

  .recent-remove {
    all: unset;
    cursor: pointer;
    width: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--mt-fg-subtle);
    opacity: 0;
    transition: opacity 120ms ease, background 120ms ease, color 120ms ease;
  }
  .recent:hover .recent-remove,
  .recent-remove:focus-visible {
    opacity: 1;
  }
  .recent-remove:hover {
    background: var(--mt-error-bg);
    color: var(--mt-error);
  }
  .recent-remove:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: -2px;
  }

  /* Inline error */
  .error {
    margin-top: 18px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: var(--mt-error-bg);
    color: var(--mt-error-fg);
    border: 1px solid color-mix(in srgb, var(--mt-error) 18%, transparent);
    border-radius: 4px;
    font-size: 13px;
    line-height: 1.5;
  }
  .error-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--mt-error);
    flex-shrink: 0;
  }
  .error-text {
    flex: 1;
    word-break: break-word;
  }
  .error-close {
    all: unset;
    cursor: pointer;
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: var(--mt-error-fg);
    opacity: 0.7;
  }
  .error-close:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--mt-error) 16%, transparent);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  /* Reduce-motion override: anime.js's onMount path already short-circuits,
     but a CSS fallback for the static .reveal initial-opacity:0 ensures the
     content is visible even before JS runs (e.g. SSR snapshot). */
  @media (prefers-reduced-motion: reduce) {
    .reveal {
      opacity: 1;
    }
    .hero-frame,
    .hero-header-band,
    .hero-line,
    .hero-glyph,
    .hero-cell,
    .hero-cursor {
      opacity: 1;
    }
    .hero-highlight {
      opacity: 0.35;
    }
  }
</style>
