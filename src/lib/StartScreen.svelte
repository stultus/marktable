<script lang="ts">
  import { pickFolder, pickFile, openFolder, openFile, type TableModel } from "./api";

  let { onOpen }: { onOpen: (t: TableModel) => void } = $props();
  let busy = $state(false);
  let error = $state<string | null>(null);
  let busyKind = $state<"folder" | "file" | null>(null);

  async function chooseFolder() {
    error = null;
    const path = await pickFolder();
    if (!path) return;
    busy = true;
    busyKind = "folder";
    try {
      const table = await openFolder(path);
      onOpen(table);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
      busyKind = null;
    }
  }

  async function chooseFile() {
    error = null;
    const path = await pickFile();
    if (!path) return;
    busy = true;
    busyKind = "file";
    try {
      const table = await openFile(path);
      onOpen(table);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
      busyKind = null;
    }
  }
</script>

<main class="start" data-busy={busy}>
  <header class="topbar">
    <div class="brand">
      <span class="brand-mark" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="16" height="16" fill="none">
          <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" stroke-width="1.4" />
          <path d="M5 2.5v11M11 2.5v11M1.5 6h13M1.5 10h13" stroke="currentColor" stroke-width="1.2" />
        </svg>
      </span>
      <span class="brand-name">MarkTable</span>
    </div>
    <span class="version">v0.1 &nbsp;·&nbsp; local-first</span>
  </header>

  <section class="hero">
    <p class="eyebrow">A quiet utility for structured content</p>
    <h1>
      Edit structured content<br />
      <span class="accent">like a spreadsheet.</span>
    </h1>
    <p class="lede">
      Open a folder of markdown frontmatter, a JSON array, or a YAML list. Each
      record becomes a row, each field a column. Edit inline, hit Save All, get
      your files back exactly as they were &mdash; only the cells you touched
      change.
    </p>
  </section>

  <section class="choices" aria-label="Open">
    <button
      type="button"
      class="choice"
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
      class="choice"
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

  <footer class="footnote">
    <span><strong>Read</strong>&nbsp;parses your files into a unified schema.</span>
    <span class="sep" aria-hidden="true">/</span>
    <span><strong>Edit</strong>&nbsp;in cells; nothing is written yet.</span>
    <span class="sep" aria-hidden="true">/</span>
    <span><strong>Save All</strong>&nbsp;writes back, original formatting preserved.</span>
  </footer>
</main>

<style>
  .start {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto 1fr auto auto;
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
  .version {
    font-family: var(--mt-font-mono);
    font-size: 11.5px;
    letter-spacing: 0.04em;
    text-transform: lowercase;
  }

  /* Hero */
  .hero {
    padding: 72px 0 36px;
    max-width: 720px;
  }
  .eyebrow {
    font-family: var(--mt-font-mono);
    font-size: 11.5px;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--mt-fg-subtle);
    margin: 0 0 20px;
  }
  h1 {
    margin: 0;
    font-family: var(--mt-font-display);
    font-weight: 600;
    font-size: clamp(34px, 4.6vw, 50px);
    line-height: 1.08;
    letter-spacing: -0.024em;
    color: var(--mt-fg);
  }
  h1 .accent {
    color: var(--mt-fg-muted);
    font-weight: 500;
  }
  .lede {
    margin: 22px 0 0;
    font-size: 15px;
    line-height: 1.65;
    color: var(--mt-fg-muted);
    max-width: 56ch;
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

  /* Footnote */
  .footnote {
    margin-top: 32px;
    padding-top: 18px;
    border-top: 1px solid var(--mt-divider);
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    color: var(--mt-fg-muted);
    font-size: 12.5px;
  }
  .footnote strong {
    color: var(--mt-fg);
    font-weight: 600;
  }
  .footnote .sep {
    color: var(--mt-fg-subtle);
    font-family: var(--mt-font-mono);
    font-size: 11px;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Page-load reveal: subtle staggered fade. Notion is restrained — short, soft. */
  .start :global(.eyebrow),
  .hero h1,
  .lede,
  .choices > .choice,
  .footnote {
    animation: rise 480ms cubic-bezier(0.2, 0.8, 0.3, 1) both;
  }
  .hero h1 {
    animation-delay: 60ms;
  }
  .lede {
    animation-delay: 120ms;
  }
  .choices > .choice:nth-child(1) {
    animation-delay: 200ms;
  }
  .choices > .choice:nth-child(2) {
    animation-delay: 260ms;
  }
  .footnote {
    animation-delay: 340ms;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .start :global(.eyebrow),
    .hero h1,
    .lede,
    .choices > .choice,
    .footnote {
      animation: none;
    }
  }
</style>
