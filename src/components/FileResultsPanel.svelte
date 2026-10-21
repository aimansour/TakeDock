<script lang="ts">
  import { onMount, untrack, tick } from 'svelte';
  import type { FileNotice } from '../lib/types';
  import { resultKey } from '../lib/file-results';
  import { t } from '../lib/i18n';
  let {
    notices,
    history,
    language,
    onHide,
  }: {
    notices: FileNotice[];
    history: FileNotice[];
    language: 'en' | 'ar';
    onHide: (notice: FileNotice) => Promise<void>;
  } = $props();
  let root = $state<HTMLElement>();
  let summary = $state<HTMLElement>();
  let rendered = $state<FileNotice[]>([]);
  let open = $state(false);
  let pinned = $state<FileNotice[]>([]);
  let disposed = false;
  function focused(key: string): boolean {
    return [
      ...(root?.querySelectorAll<HTMLElement>('.file-result-notice') ?? []),
    ].some(
      (card) =>
        card.dataset.resultKey === key && card.contains(document.activeElement),
    );
  }
  $effect.pre(() => {
    const current = notices;
    const incoming = new Map(
      current.map((notice) => [resultKey(notice.key), notice]),
    );
    const previous = untrack(() => rendered);
    const previousKeys = new Set(
      previous.map((notice) => resultKey(notice.key)),
    );
    // Moving a focused card to the end detaches its native focus in Chromium.
    // Keep surviving cards in their existing slots; prepend only new results.
    rendered = [
      ...current.filter((notice) => !previousKeys.has(resultKey(notice.key))),
      ...previous.flatMap((notice) => {
        const key = resultKey(notice.key);
        const fresh = incoming.get(key);
        return fresh ? [fresh] : focused(key) ? [notice] : [];
      }),
    ];
  });
  $effect.pre(() => {
    const incoming = history;
    const keys = new Set(incoming.map((notice) => resultKey(notice.key)));
    const previous = untrack(() => pinned);
    pinned = open
      ? [
          ...incoming,
          ...previous.filter((notice) => !keys.has(resultKey(notice.key))),
        ]
      : [...incoming];
  });
  function retire() {
    if (disposed) return;
    const keys = new Set(notices.map((notice) => resultKey(notice.key)));
    rendered = rendered.filter(
      (notice) =>
        keys.has(resultKey(notice.key)) || focused(resultKey(notice.key)),
    );
  }
  onMount(() => {
    const focusout = () => queueMicrotask(retire);
    root?.addEventListener('focusout', focusout);
    return () => {
      disposed = true;
      root?.removeEventListener('focusout', focusout);
    };
  });
  async function hide(notice: FileNotice, button: HTMLButtonElement) {
    try {
      await onHide(notice);
    } catch {
      return;
    }
    await tick();
    const key = resultKey(notice.key);
    if (
      disposed ||
      document.activeElement !== button ||
      notices.some((item) => resultKey(item.key) === key)
    )
      return;
    const buttons = [
      ...(root?.querySelectorAll<HTMLButtonElement>(
        '.file-result-notice button',
      ) ?? []),
    ];
    const index = buttons.indexOf(button);
    (buttons[index + 1] ?? buttons[index - 1] ?? summary)?.focus();
    retire();
  }
</script>

{#snippet outcome(notice: FileNotice)}
  <h3>{t(language, notice.kind)} · {notice.completed}/{notice.count}</h3>
  <p>
    {t(language, notice.outcome)}{#if notice.name}
      · <bdi>{notice.name}</bdi>{/if}
  </p>
  {#if notice.code && notice.code !== notice.outcome}<p>
      {t(language, notice.code)}
    </p>{/if}
  {#each notice.warnings as warning}<p>{t(language, warning)}</p>{/each}
  {#if notice.results.length}<ul>
      {#each notice.results as item}<li>
          <p>
            <bdi>{item.name}</bdi>{#if item.destination_name}
              → <bdi data-committed-name>{item.destination_name}</bdi>{/if}: {t(
              language,
              item.status,
            )}
          </p>
          {#if item.requested_name && !item.destination_name}<p>
              {t(language, 'requestedName')}: <bdi>{item.requested_name}</bdi>
            </p>{/if}
          {#if item.message}<p>{t(language, item.message)}</p>{/if}
        </li>{/each}
    </ul>{/if}
{/snippet}

<section
  class="panel file-results-panel"
  bind:this={root}
  dir={language === 'ar' ? 'rtl' : 'ltr'}
>
  <h2>{t(language, 'fileResults')}</h2>
  {#each rendered as notice (resultKey(notice.key))}
    <article
      class="job file-result-notice"
      data-result-key={resultKey(notice.key)}
      data-operation-id={notice.key.operation_id}
    >
      {@render outcome(notice)}
      <button onclick={(event) => void hide(notice, event.currentTarget)}
        >{t(language, 'hideFileResult')} · {t(language, notice.kind)} ·
        <bdi>{notice.name}</bdi></button
      >
    </article>
  {/each}
  <details
    class="file-history"
    ontoggle={(event) => (open = event.currentTarget.open)}
  >
    <summary bind:this={summary}>{t(language, 'operationHistory')}</summary>
    {#each pinned as notice (resultKey(notice.key))}<article class="job">
        {@render outcome(notice)}
      </article>
    {:else}<p>{t(language, 'noFileHistory')}</p>{/each}
  </details>
</section>
