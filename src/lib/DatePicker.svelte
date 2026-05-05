<script lang="ts">
  import { tick } from "svelte";

  let {
    value,
    onChange,
    placeholder = "—",
  }: {
    value: string;
    onChange: (v: string) => void;
    placeholder?: string;
  } = $props();

  let open = $state(false);
  let trigger = $state<HTMLButtonElement | null>(null);
  let popover = $state<HTMLDivElement | null>(null);

  function parsed(): Date | null {
    if (!value) return null;
    const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(value);
    if (!m) return null;
    const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
    return Number.isFinite(d.getTime()) ? d : null;
  }

  function todayLocal(): Date {
    const now = new Date();
    return new Date(now.getFullYear(), now.getMonth(), now.getDate());
  }

  let view = $state(parsed() ?? todayLocal());

  function fmtIso(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }

  function fmtDisplay(d: Date | null): string {
    if (!d) return placeholder;
    return d.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  }

  const monthNames = [
    "January","February","March","April","May","June",
    "July","August","September","October","November","December",
  ];
  const dayNames = ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];

  // Build a 6×7 matrix of Date cells starting on Monday.
  function gridFor(year: number, month: number): Date[] {
    const first = new Date(year, month, 1);
    // 0=Sun..6=Sat → shift so Mon=0
    const offset = (first.getDay() + 6) % 7;
    const start = new Date(year, month, 1 - offset);
    const cells: Date[] = [];
    for (let i = 0; i < 42; i++) {
      cells.push(new Date(start.getFullYear(), start.getMonth(), start.getDate() + i));
    }
    return cells;
  }

  let grid = $derived(gridFor(view.getFullYear(), view.getMonth()));

  function sameDay(a: Date, b: Date): boolean {
    return (
      a.getFullYear() === b.getFullYear() &&
      a.getMonth() === b.getMonth() &&
      a.getDate() === b.getDate()
    );
  }

  function isInMonth(d: Date): boolean {
    return d.getMonth() === view.getMonth();
  }

  async function toggle() {
    open = !open;
    if (open) {
      view = parsed() ?? todayLocal();
      await tick();
    }
  }

  function close() {
    open = false;
    trigger?.focus();
  }

  function pick(d: Date) {
    onChange(fmtIso(d));
    open = false;
    trigger?.focus();
  }

  function clear() {
    onChange("");
    open = false;
    trigger?.focus();
  }

  function pickToday() {
    pick(todayLocal());
  }

  function nudgeMonth(delta: number) {
    view = new Date(view.getFullYear(), view.getMonth() + delta, 1);
  }
  function nudgeYear(delta: number) {
    view = new Date(view.getFullYear() + delta, view.getMonth(), 1);
  }

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  function onWindowMouse(e: MouseEvent) {
    const target = e.target as Node;
    if (trigger?.contains(target)) return;
    if (popover?.contains(target)) return;
    open = false;
  }

  $effect(() => {
    if (open) {
      window.addEventListener("mousedown", onWindowMouse);
      window.addEventListener("keydown", onKey);
      return () => {
        window.removeEventListener("mousedown", onWindowMouse);
        window.removeEventListener("keydown", onKey);
      };
    }
  });

  let picked = $derived(parsed());
</script>

<div class="dp-shell">
  <button
    bind:this={trigger}
    type="button"
    class="dp-trigger"
    class:placeholder={!picked}
    aria-haspopup="dialog"
    aria-expanded={open}
    onclick={toggle}
  >
    <span class="dp-value">{fmtDisplay(picked)}</span>
    <span class="dp-icon" aria-hidden="true">
      <svg viewBox="0 0 16 16" width="13" height="13" fill="none">
        <rect x="2.4" y="3.6" width="11.2" height="10" rx="1.4" stroke="currentColor" stroke-width="1.4"/>
        <path d="M2.4 6.6h11.2M5.6 2v3.2M10.4 2v3.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
      </svg>
    </span>
  </button>

  {#if open}
    <div bind:this={popover} class="dp-popover" role="dialog" aria-label="Choose a date">
      <div class="dp-head">
        <button type="button" class="dp-nav" onclick={() => nudgeYear(-1)} aria-label="Previous year">
          <svg viewBox="0 0 12 12" width="11" height="11" fill="none">
            <path d="M9 3l-3 3 3 3M5.5 3l-3 3 3 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <button type="button" class="dp-nav" onclick={() => nudgeMonth(-1)} aria-label="Previous month">
          <svg viewBox="0 0 12 12" width="11" height="11" fill="none">
            <path d="M7.5 3l-3 3 3 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <span class="dp-title">
          {monthNames[view.getMonth()]} <span class="dp-year">{view.getFullYear()}</span>
        </span>
        <button type="button" class="dp-nav" onclick={() => nudgeMonth(1)} aria-label="Next month">
          <svg viewBox="0 0 12 12" width="11" height="11" fill="none">
            <path d="M4.5 3l3 3-3 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <button type="button" class="dp-nav" onclick={() => nudgeYear(1)} aria-label="Next year">
          <svg viewBox="0 0 12 12" width="11" height="11" fill="none">
            <path d="M3 3l3 3-3 3M6.5 3l3 3-3 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>

      <div class="dp-weekdays" aria-hidden="true">
        {#each dayNames as d (d)}
          <span>{d}</span>
        {/each}
      </div>

      <div class="dp-grid" role="grid">
        {#each grid as d (d.toISOString())}
          <button
            type="button"
            class="dp-day"
            class:dim={!isInMonth(d)}
            class:today={sameDay(d, todayLocal())}
            class:selected={picked && sameDay(d, picked)}
            onclick={() => pick(d)}
            tabindex={isInMonth(d) ? 0 : -1}
            aria-label={d.toLocaleDateString(undefined, { dateStyle: "full" })}
          >
            <span>{d.getDate()}</span>
          </button>
        {/each}
      </div>

      <div class="dp-footer">
        <button type="button" class="dp-quick" onclick={pickToday}>Today</button>
        {#if picked}
          <button type="button" class="dp-quick danger" onclick={clear}>Clear</button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .dp-shell {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
  }

  /* Trigger — sits inside the table cell. Looks like a normal cell on hover,
     not a button, so it doesn't shout in the grid. */
  .dp-trigger {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 100%;
    min-height: 36px;
    padding: 8px 12px;
    color: var(--mt-fg);
    font: inherit;
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .dp-trigger.placeholder .dp-value {
    color: var(--mt-fg-subtle);
  }
  .dp-trigger:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: -2px;
    border-radius: 0;
  }
  .dp-icon {
    display: inline-flex;
    color: var(--mt-fg-subtle);
    margin-left: 8px;
    transition: color 160ms ease, transform 200ms ease;
  }
  .dp-trigger:hover .dp-icon {
    color: var(--mt-fg-muted);
  }
  .dp-trigger[aria-expanded="true"] .dp-icon {
    color: var(--mt-accent);
    transform: scale(1.05);
  }

  /* Popover */
  .dp-popover {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 1100;
    width: 280px;
    background: var(--mt-elevated);
    border: 1px solid var(--mt-border-strong);
    border-radius: 10px;
    padding: 10px;
    box-shadow: var(--mt-shadow-3);
    animation: dp-in 180ms cubic-bezier(0.2, 0.9, 0.3, 1.05);
  }
  @keyframes dp-in {
    from { opacity: 0; transform: translateY(-4px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  .dp-head {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-bottom: 6px;
  }
  .dp-title {
    flex: 1;
    text-align: center;
    font-family: var(--mt-font-display);
    font-weight: 600;
    font-size: 13.5px;
    color: var(--mt-fg);
    letter-spacing: -0.005em;
    user-select: none;
  }
  .dp-year {
    color: var(--mt-fg-subtle);
    font-weight: 500;
    margin-left: 2px;
  }
  .dp-nav {
    all: unset;
    cursor: pointer;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--mt-fg-muted);
    transition: background 120ms ease, color 120ms ease;
  }
  .dp-nav:hover {
    background: var(--mt-hover);
    color: var(--mt-fg);
  }
  .dp-nav:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }

  .dp-weekdays {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
    padding: 4px 0 6px;
    font-family: var(--mt-font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mt-fg-subtle);
    text-align: center;
    border-bottom: 1px solid var(--mt-divider);
    margin-bottom: 6px;
  }

  .dp-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }

  .dp-day {
    all: unset;
    cursor: pointer;
    aspect-ratio: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-family: var(--mt-font-mono);
    font-size: 12px;
    color: var(--mt-fg);
    border-radius: 5px;
    user-select: none;
    transition:
      background 120ms ease,
      color 120ms ease,
      box-shadow 120ms ease;
    position: relative;
  }
  .dp-day:hover {
    background: var(--mt-hover);
  }
  .dp-day.dim {
    color: var(--mt-fg-subtle);
    opacity: 0.55;
  }
  .dp-day.today {
    color: var(--mt-accent);
    font-weight: 600;
  }
  .dp-day.today::after {
    content: "";
    position: absolute;
    bottom: 4px;
    left: 50%;
    transform: translateX(-50%);
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--mt-accent);
  }
  .dp-day.selected {
    background: var(--mt-accent);
    color: #fff;
    box-shadow: 0 1px 2px rgba(35, 131, 226, 0.3);
  }
  .dp-day.selected::after {
    background: #fff;
  }
  .dp-day:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }

  .dp-footer {
    display: flex;
    gap: 6px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--mt-divider);
  }
  .dp-quick {
    all: unset;
    cursor: pointer;
    flex: 1;
    text-align: center;
    padding: 6px 10px;
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mt-fg-muted);
    border: 1px solid var(--mt-divider);
    border-radius: 5px;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }
  .dp-quick:hover {
    background: var(--mt-hover);
    color: var(--mt-fg);
    border-color: var(--mt-border-strong);
  }
  .dp-quick.danger:hover {
    color: var(--mt-error);
    border-color: color-mix(in srgb, var(--mt-error) 35%, transparent);
    background: var(--mt-error-bg);
  }
  .dp-quick:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }
</style>
