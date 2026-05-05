<script lang="ts">
  import { tick } from "svelte";

  let {
    options,
    value,
    onChange,
    label,
  }: {
    options: { value: string; label: string; hint?: string }[];
    value: string;
    onChange: (v: string) => void;
    label?: string;
  } = $props();

  let open = $state(false);
  let trigger = $state<HTMLButtonElement | null>(null);
  let menu = $state<HTMLUListElement | null>(null);
  let activeIndex = $state(-1);

  let current = $derived(options.find((o) => o.value === value));

  async function toggleOpen() {
    open = !open;
    if (open) {
      activeIndex = options.findIndex((o) => o.value === value);
      await tick();
      menu?.querySelector<HTMLLIElement>(`li[data-active="true"]`)?.focus();
    }
  }

  function select(v: string) {
    onChange(v);
    open = false;
    trigger?.focus();
  }

  function onKey(e: KeyboardEvent) {
    if (!open) {
      if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
        e.preventDefault();
        toggleOpen();
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      open = false;
      trigger?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      activeIndex = (activeIndex + 1) % options.length;
      focusActive();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      activeIndex = (activeIndex - 1 + options.length) % options.length;
      focusActive();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const opt = options[activeIndex];
      if (opt) select(opt.value);
    }
  }

  async function focusActive() {
    await tick();
    menu?.querySelector<HTMLLIElement>(`li[data-active="true"]`)?.focus();
  }

  function onWindowClick(e: MouseEvent) {
    const target = e.target as Node;
    if (trigger?.contains(target)) return;
    if (menu?.contains(target)) return;
    open = false;
  }

  $effect(() => {
    if (open) {
      window.addEventListener("mousedown", onWindowClick);
      return () => window.removeEventListener("mousedown", onWindowClick);
    }
  });
</script>

<div class="select-shell" role="presentation">
  <button
    bind:this={trigger}
    type="button"
    class="select-trigger"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
    onclick={toggleOpen}
    onkeydown={onKey}
  >
    <span class="select-value">{current?.label ?? value}</span>
    <span class="select-chevron" aria-hidden="true">
      <svg viewBox="0 0 12 12" width="11" height="11" fill="none">
        <path
          d="M3 5l3 3 3-3"
          stroke="currentColor"
          stroke-width="1.4"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </span>
  </button>

  {#if open}
    <ul
      bind:this={menu}
      class="select-menu"
      role="listbox"
      tabindex="-1"
      onkeydown={onKey}
    >
      {#each options as opt, i (opt.value)}
        <li
          class="select-option"
          class:selected={opt.value === value}
          data-active={i === activeIndex}
          role="option"
          aria-selected={opt.value === value}
          tabindex="-1"
          onclick={() => select(opt.value)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              select(opt.value);
            }
          }}
          onmouseenter={() => (activeIndex = i)}
        >
          <span class="select-option-label">{opt.label}</span>
          {#if opt.hint}
            <span class="select-option-hint">{opt.hint}</span>
          {/if}
          {#if opt.value === value}
            <span class="select-option-check" aria-hidden="true">
              <svg viewBox="0 0 12 12" width="11" height="11" fill="none">
                <path
                  d="M2.5 6.4l2.4 2.3L9.5 3.6"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .select-shell {
    position: relative;
    display: block;
    width: 100%;
  }

  .select-trigger {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 9px 12px;
    background: var(--mt-surface);
    color: var(--mt-fg);
    border: 1px solid var(--mt-border);
    border-radius: 6px;
    font: inherit;
    font-size: 14px;
    transition:
      border-color 140ms ease,
      background 140ms ease,
      box-shadow 140ms ease;
  }
  .select-trigger:hover {
    background: var(--mt-elevated);
    border-color: var(--mt-border-strong);
  }
  .select-trigger:focus-visible,
  .select-trigger[aria-expanded="true"] {
    border-color: var(--mt-accent);
    background: var(--mt-elevated);
    box-shadow: 0 0 0 3px var(--mt-accent-soft);
  }
  .select-value {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .select-chevron {
    color: var(--mt-fg-subtle);
    margin-left: 8px;
    display: inline-flex;
    transition: transform 200ms ease;
  }
  .select-trigger[aria-expanded="true"] .select-chevron {
    transform: rotate(180deg);
    color: var(--mt-accent);
  }

  .select-menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 1100;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--mt-elevated);
    border: 1px solid var(--mt-border-strong);
    border-radius: 8px;
    box-shadow: var(--mt-shadow-3);
    max-height: 280px;
    overflow-y: auto;
    animation: menu-in 160ms cubic-bezier(0.2, 0.9, 0.3, 1.05);
  }
  @keyframes menu-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .select-option {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 5px;
    font-size: 13.5px;
    color: var(--mt-fg);
    cursor: pointer;
    user-select: none;
    transition: background 100ms ease, color 100ms ease;
  }
  .select-option:hover,
  .select-option[data-active="true"] {
    background: var(--mt-hover);
  }
  .select-option.selected {
    color: var(--mt-accent);
  }
  .select-option-label {
    flex: 1;
  }
  .select-option-hint {
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mt-fg-subtle);
  }
  .select-option-check {
    color: var(--mt-accent);
    display: inline-flex;
  }
</style>
