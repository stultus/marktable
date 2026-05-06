<script lang="ts">
  import { onMount } from "svelte";
  import StartScreen from "$lib/StartScreen.svelte";
  import TableView from "$lib/TableView.svelte";
  import type { TableModel } from "$lib/api";

  let table = $state<TableModel | null>(null);
  let fatal = $state<string | null>(null);

  onMount(() => {
    const onError = (e: ErrorEvent) => {
      fatal = `${e.message}\n${e.error?.stack ?? ""}`;
      console.error("[marktable] window error", e.error ?? e.message);
    };
    const onRejection = (e: PromiseRejectionEvent) => {
      const reason = e.reason;
      fatal = typeof reason === "string"
        ? reason
        : (reason?.message ?? JSON.stringify(reason));
      console.error("[marktable] unhandled rejection", reason);
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  });
</script>

<svelte:head>
  <title>MarkTable</title>
</svelte:head>

{#if fatal !== null}
  <div class="fatal-backdrop" onclick={() => (fatal = null)} role="presentation"></div>
  <div class="fatal" role="alertdialog" aria-labelledby="fatal-title">
    <div class="fatal-head">
      <span class="dot" aria-hidden="true"></span>
      <h2 id="fatal-title">Something went wrong</h2>
    </div>
    <pre>{fatal}</pre>
    <div class="fatal-actions">
      <button onclick={() => (fatal = null)}>Dismiss</button>
    </div>
  </div>
{/if}

{#if table === null}
  <StartScreen onOpen={(t) => (table = t)} />
{:else}
  <TableView {table} onClose={() => (table = null)} />
{/if}

<style>
  .fatal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 15, 15, 0.32);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    z-index: 999;
    animation: fade-in 140ms ease-out;
  }
  .fatal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    background: var(--mt-elevated);
    color: var(--mt-fg);
    border: 1px solid var(--mt-border-strong);
    border-radius: 8px;
    padding: 20px 22px 16px;
    z-index: 1000;
    box-shadow: var(--mt-shadow-2);
    display: flex;
    flex-direction: column;
    animation: pop-in 180ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  .fatal-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 0 8px;
  }
  .fatal-head .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--mt-error);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--mt-error) 22%, transparent);
  }
  .fatal h2 {
    font-family: var(--mt-font-display);
    font-weight: 600;
    font-size: 18px;
    margin: 0;
    letter-spacing: -0.01em;
    color: var(--mt-fg);
  }
  .fatal pre {
    flex: 1;
    overflow: auto;
    margin: 6px 0 14px;
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    line-height: 1.5;
    background: var(--mt-surface);
    border: 1px solid var(--mt-divider);
    border-radius: 4px;
    padding: 10px 12px;
    color: var(--mt-fg-muted);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 50vh;
  }
  .fatal-actions {
    display: flex;
    justify-content: flex-end;
  }
  .fatal-actions button {
    padding: 6px 14px;
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    border-radius: 4px;
    border: 1px solid var(--mt-border-strong);
    background: var(--mt-surface);
    color: var(--mt-fg);
    cursor: pointer;
    transition: background 120ms ease;
  }
  .fatal-actions button:hover {
    background: var(--mt-hover);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translate(-50%, -48%) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1);
    }
  }

  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: var(--mt-page-bg);
    color: var(--mt-fg);
    font-family: var(--mt-font-sans);
    font-size: 14px;
    line-height: 1.5;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;
    font-feature-settings:
      "ss01" on,
      "cv11" on;
  }
  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }
  /* Global reduced-motion shutdown. Honors the OS-level setting per WCAG 2.3.3.
     Animation-heavy components (DatePicker pop-in, Select menu fade, modal
     pop-in, dirty-pill pulse, toast slide) all disable themselves here without
     each one having to opt in. */
  @media (prefers-reduced-motion: reduce) {
    :global(*, *::before, *::after) {
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: 0.01ms !important;
      scroll-behavior: auto !important;
    }
  }
  :global(::selection) {
    background: var(--mt-accent-soft);
    color: var(--mt-fg);
  }
  /* Design tokens (--mt-*) are defined in src/lib/theme.css and imported
     once globally from +layout.ts. */
</style>
