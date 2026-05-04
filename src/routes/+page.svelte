<script lang="ts">
  import { onMount } from "svelte";
  import StartScreen from "$lib/StartScreen.svelte";
  import TableView from "$lib/TableView.svelte";
  import { demoJson, demoYaml } from "$lib/demo";
  import type { TableModel } from "$lib/api";

  let table = $state<TableModel | null>(null);
  let fatal = $state<string | null>(null);

  onMount(() => {
    // ?demo=json | ?demo=yaml — bypasses Tauri commands so the UI can be
    // verified in plain Chrome (headless or otherwise).
    const params = new URLSearchParams(window.location.search);
    const demo = params.get("demo");
    if (demo === "json") table = demoJson();
    else if (demo === "yaml") table = demoYaml();

    const onError = (e: ErrorEvent) => {
      // Browsers fire this benign warning when a ResizeObserver callback
      // schedules another layout change in the same frame. It is not an
      // actual error and should not surface as a fatal modal.
      if (
        e.message?.startsWith("ResizeObserver loop") ||
        e.message?.includes("ResizeObserver loop completed")
      ) {
        return;
      }
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
  :global(::selection) {
    background: var(--mt-accent-soft);
    color: var(--mt-fg);
  }
  :global(:root) {
    /* Typography — developer-tool stack */
    --mt-font-sans:
      "Geist", "SF Pro Text", -apple-system, BlinkMacSystemFont, "Segoe UI",
      "Helvetica Neue", sans-serif;
    --mt-font-mono:
      "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, "Cascadia Mono",
      Consolas, monospace;
    --mt-font-display:
      "Geist", "SF Pro Display", -apple-system, BlinkMacSystemFont, "Segoe UI",
      sans-serif;

    /* Surfaces — Notion warm-paper palette */
    --mt-page-bg: #ffffff;
    --mt-surface: #fbfaf9;
    --mt-surface-strong: #f1f1ef;
    --mt-elevated: #ffffff;

    /* Text — warm near-black, not pure black */
    --mt-fg: #37352f;
    --mt-fg-muted: #787774;
    --mt-fg-subtle: #9b9a97;

    /* Lines */
    --mt-border: rgba(55, 53, 47, 0.09);
    --mt-border-strong: rgba(55, 53, 47, 0.16);
    --mt-divider: rgba(55, 53, 47, 0.07);

    /* States */
    --mt-hover: rgba(55, 53, 47, 0.05);
    --mt-active: rgba(55, 53, 47, 0.10);

    /* Accent */
    --mt-accent: #2383e2;
    --mt-accent-hover: #0b6bcb;
    --mt-accent-soft: rgba(35, 131, 226, 0.10);

    /* Tag pills (light) */
    --mt-tag-blue: #d3e5ef;
    --mt-tag-blue-fg: #183347;
    --mt-tag-green: #ddedea;
    --mt-tag-green-fg: #1c3735;
    --mt-tag-yellow: #fdecc8;
    --mt-tag-yellow-fg: #5c3b00;
    --mt-tag-red: #ffe2dd;
    --mt-tag-red-fg: #5d1715;
    --mt-tag-purple: #e8deee;
    --mt-tag-purple-fg: #492f64;
    --mt-tag-pink: #f5e0e9;
    --mt-tag-pink-fg: #4f3037;
    --mt-tag-orange: #fadec9;
    --mt-tag-orange-fg: #5c3b23;
    --mt-tag-gray: #ebebe9;
    --mt-tag-gray-fg: #50504d;

    /* Semantic */
    --mt-warn: #cb912f;
    --mt-warn-bg: #fbecdd;
    --mt-warn-fg: #5c3b00;
    --mt-error: #e03e3e;
    --mt-error-bg: #fbe4e4;
    --mt-error-fg: #5d1715;
    --mt-success: #0f7b6c;
    --mt-success-bg: #ddedea;
    --mt-success-fg: #1c3735;

    /* Shadows */
    --mt-shadow-1: 0 1px 2px rgba(15, 15, 15, 0.05),
      0 2px 4px rgba(15, 15, 15, 0.04);
    --mt-shadow-2: 0 1px 3px rgba(15, 15, 15, 0.08),
      0 6px 16px rgba(15, 15, 15, 0.06);
    --mt-shadow-3: 0 8px 32px rgba(15, 15, 15, 0.18),
      0 2px 6px rgba(15, 15, 15, 0.08);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --mt-page-bg: #191919;
      --mt-surface: #202020;
      --mt-surface-strong: #2a2a2a;
      --mt-elevated: #2f2f2f;

      --mt-fg: #e6e6e4;
      --mt-fg-muted: #9b9b9b;
      --mt-fg-subtle: #6f6f6f;

      --mt-border: rgba(255, 255, 255, 0.094);
      --mt-border-strong: rgba(255, 255, 255, 0.18);
      --mt-divider: rgba(255, 255, 255, 0.07);

      --mt-hover: rgba(255, 255, 255, 0.055);
      --mt-active: rgba(255, 255, 255, 0.10);

      --mt-accent: #529cca;
      --mt-accent-hover: #80b8df;
      --mt-accent-soft: rgba(82, 156, 202, 0.18);

      --mt-tag-blue: #1f3954;
      --mt-tag-blue-fg: #5a9bd1;
      --mt-tag-green: #1c3735;
      --mt-tag-green-fg: #5fbeae;
      --mt-tag-yellow: #56452f;
      --mt-tag-yellow-fg: #e9c167;
      --mt-tag-red: #3a2222;
      --mt-tag-red-fg: #ff7369;
      --mt-tag-purple: #2f2245;
      --mt-tag-purple-fg: #b18bd0;
      --mt-tag-pink: #3b2735;
      --mt-tag-pink-fg: #ec98c9;
      --mt-tag-orange: #4a2f1d;
      --mt-tag-orange-fg: #ed9054;
      --mt-tag-gray: #2a2a2a;
      --mt-tag-gray-fg: #9b9b9b;

      --mt-warn: #dfab01;
      --mt-warn-bg: #3a2f1f;
      --mt-warn-fg: #f0c462;
      --mt-error: #ff7369;
      --mt-error-bg: #3a2222;
      --mt-error-fg: #ff9d8b;
      --mt-success: #4dab9a;
      --mt-success-bg: #1c3735;
      --mt-success-fg: #6cc6b3;

      --mt-shadow-1: 0 1px 2px rgba(0, 0, 0, 0.4),
        0 2px 4px rgba(0, 0, 0, 0.3);
      --mt-shadow-2: 0 1px 3px rgba(0, 0, 0, 0.5),
        0 8px 24px rgba(0, 0, 0, 0.45);
      --mt-shadow-3: 0 16px 40px rgba(0, 0, 0, 0.6),
        0 2px 8px rgba(0, 0, 0, 0.4);
    }
  }
</style>
