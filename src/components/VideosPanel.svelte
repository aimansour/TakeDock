<script lang="ts">
  import type { Video, JobKind } from '../lib/types';
  import { tick } from 'svelte';
  import { t } from '../lib/i18n';
  import { videoKey } from '../lib/videos';
  import { duration } from '../lib/duration';
  let {
    videos = [],
    onJob,
    language = 'en',
    connected = true,
    onRefresh,
    loading = false,
    generation = 0,
    pendingKeys = [],
  }: {
    videos: Video[];
    onJob: (
      kind: JobKind,
      videos: Video[],
      stem?: string,
      generation?: number,
    ) => void;
    language?: 'en' | 'ar';
    connected?: boolean;
    onRefresh: () => void;
    loading?: boolean;
    generation?: number;
    pendingKeys?: string[];
  } = $props();
  let selected = $state<string[]>([]);
  let renaming = $state(false);
  let stem = $state('');
  let deleting = $state<Video[]>([]);
  let renameInput = $state<HTMLInputElement>();
  let confirmation = $state<HTMLButtonElement>();
  let toolbar = $state<HTMLButtonElement>();
  let table = $state<HTMLTableElement>();
  let actionToolbar = $state<HTMLDivElement>();
  let renderedKeys: string[] = [];
  function pending(video: Video) {
    return pendingKeys.includes(videoKey(video));
  }
  function moveRemovedFocus(
    key: string | undefined,
    original: Element,
    preferButton: boolean,
  ) {
    void tick().then(() => {
      if (!toolbar?.isConnected) return;
      if (
        document.activeElement !== document.body &&
        document.activeElement !== original
      )
        return;
      const row =
        key &&
        Array.from(
          table?.querySelectorAll<HTMLTableRowElement>('tr[data-video-key]') ??
            [],
        ).find((row) => row.dataset.videoKey === key);
      const control =
        row &&
        (row.querySelector<HTMLElement>(
          preferButton
            ? 'button[data-mark]:not(:disabled)'
            : 'input:not(:disabled)',
        ) ??
          row.querySelector<HTMLElement>(
            'input:not(:disabled),button:not(:disabled)',
          ));
      (control || toolbar).focus();
    });
  }
  $effect.pre(() => {
    const keys = videos.map(videoKey);
    const active = document.activeElement;
    const row =
      active instanceof HTMLElement
        ? active.closest<HTMLTableRowElement>('tr[data-video-key]')
        : null;
    if (
      active &&
      row &&
      table?.contains(row) &&
      !keys.includes(row.dataset.videoKey ?? '')
    ) {
      const position = Math.max(
        0,
        renderedKeys.indexOf(row.dataset.videoKey ?? ''),
      );
      const next = [
        ...videos.slice(position),
        ...videos.slice(0, position).reverse(),
      ].find((video) => video.ready && connected && !pending(video));
      moveRemovedFocus(
        next && videoKey(next),
        active,
        active.tagName === 'BUTTON',
      );
    } else if (
      active &&
      actionToolbar?.contains(active) &&
      !videos.some((video) => video.ready && selected.includes(videoKey(video)))
    ) {
      moveRemovedFocus(undefined, active, true);
    }
    renderedKeys = keys;
  });
  let selectionGeneration = $state(0);
  $effect.pre(() => {
    if (generation !== selectionGeneration || !connected) {
      const formFocused =
        document.activeElement === confirmation ||
        document.activeElement === renameInput;
      selected = [];
      deleting = [];
      renaming = false;
      selectionGeneration = generation;
      if (formFocused) void tick().then(() => toolbar?.focus());
    }
  });
  function job(kind: JobKind, videos: Video[], stem?: string) {
    if (
      !connected ||
      selectionGeneration !== generation ||
      !videos.length ||
      videos.some((video) => !video.ready || pending(video))
    )
      return;
    onJob(kind, videos, stem, selectionGeneration);
  }
  const chosen = $derived(
    videos.filter((video) => video.ready && selected.includes(videoKey(video))),
  );
  $effect(() => {
    const retained = selected.filter((key) =>
      videos.some((video) => video.ready && videoKey(video) === key),
    );
    if (retained.length !== selected.length) selected = retained;
  });
  function toggle(video: Video) {
    const key = videoKey(video);
    selected = selected.includes(key)
      ? selected.filter((item) => item !== key)
      : [...selected, key];
  }
  function openRename() {
    stem = chosen[0].name.replace(/\.[^.]+$/, '');
    renaming = true;
    void tick().then(() => {
      renameInput?.focus();
      renameInput?.select();
    });
  }
  function closeForm() {
    renaming = false;
    deleting = [];
    void tick().then(() => toolbar?.focus());
  }
</script>

<section class="panel" dir={language === 'ar' ? 'rtl' : 'ltr'}>
  <div class="panel-heading">
    <h2>{t(language, 'videos')}</h2>
    <button bind:this={toolbar} onclick={onRefresh}
      >{t(language, 'refresh')}</button
    >
  </div>
  <p class="hint">{t(language, 'markHelp')}</p>
  {#if !connected}<p>{t(language, 'noDevice')}</p>{/if}
  {#if loading}<p>{t(language, 'loading')}</p>{/if}
  {#if connected && videos.some((video) => video.ready)}
    <div class="toolbar">
      <button
        onclick={() =>
          (selected = videos
            .filter((video) => video.ready && !pending(video))
            .map(videoKey))}>{t(language, 'selectAll')}</button
      >{#if selected.length}<button onclick={() => (selected = [])}
          >{t(language, 'clear')}</button
        >{/if}
    </div>
  {/if}
  {#if connected && chosen.length}
    <p>{t(language, 'selected')}: {chosen.length}</p>
    <div class="toolbar" bind:this={actionToolbar}>
      <button
        disabled={chosen.some(pending)}
        onclick={() => job('mark', chosen)}
        >{t(language, 'markSelected')}</button
      >
      <button
        class="danger"
        disabled={chosen.some(pending)}
        onclick={() => {
          deleting = [...chosen];
          void tick().then(() => confirmation?.focus());
        }}>{t(language, 'delete')}</button
      >
      {#if chosen.length === 1}<button
          disabled={chosen.some(pending)}
          onclick={openRename}>{t(language, 'rename')}</button
        >{/if}
    </div>
  {/if}
  {#if renaming && chosen.length === 1}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        job('rename', [...chosen], stem);
        closeForm();
      }}
    >
      <fieldset>
        <legend>{t(language, 'rename')}</legend>
        <label for="new-name">{t(language, 'newName')}</label><input
          id="new-name"
          bind:this={renameInput}
          bind:value={stem}
          required
          maxlength="230"
        />
        <div class="toolbar">
          <button type="submit">{t(language, 'saveRename')}</button><button
            type="button"
            onclick={closeForm}>{t(language, 'cancel')}</button
          >
        </div>
      </fieldset>
    </form>
  {/if}
  {#if deleting.length}
    <fieldset>
      <legend>{t(language, 'deleteHelp')}</legend>
      <p>{deleting.length} {t(language, 'count')}</p>
      <ul>
        {#each deleting as video (videoKey(video))}<li>
            <bdi>{video.name}</bdi>
          </li>{/each}
      </ul>
      <div class="toolbar">
        <button
          class="danger"
          bind:this={confirmation}
          onclick={() => {
            job('delete', deleting);
            closeForm();
          }}>{t(language, 'confirmDelete')}</button
        ><button onclick={closeForm}>{t(language, 'cancel')}</button>
      </div>
    </fieldset>
  {/if}
  {#if videos.length}
    <div class="table-scroll">
      <table bind:this={table}>
        <thead
          ><tr
            ><th scope="col">{t(language, 'select')}</th><th scope="col"
              >{t(language, 'name')}</th
            ><th scope="col">{t(language, 'size')}</th><th scope="col"
              >{t(language, 'modified')}</th
            ><th scope="col">{t(language, 'duration')}</th><th scope="col"
              >{t(language, 'state')}</th
            ><th scope="col">{t(language, 'actions')}</th></tr
          ></thead
        ><tbody>
          {#each videos as video (videoKey(video))}<tr
              data-video-key={videoKey(video)}
              ><td
                >{#if video.ready && connected}<label
                    ><input
                      type="checkbox"
                      disabled={pending(video)}
                      checked={selected.includes(videoKey(video))}
                      onchange={() => toggle(video)}
                    /><span class="sr-only"
                      >{t(language, 'select')} {video.name}</span
                    ></label
                  >{/if}</td
              ><td><bdi>{video.name}</bdi></td><td
                >{(video.size / 1048576).toLocaleString(language, {
                  maximumFractionDigits: 1,
                })} MB</td
              ><td>{new Date(video.modified_ms).toLocaleString(language)}</td
              ><td>{duration(video.duration_ms, language)}</td><td
                >{t(
                  language,
                  pending(video)
                    ? 'running'
                    : video.ready
                      ? 'complete'
                      : 'protected',
                )}</td
              ><td
                ><button
                  type="button"
                  data-mark
                  disabled={!video.ready || !connected || pending(video)}
                  onclick={() => job('mark', [video])}
                  >{t(language, 'mark')}{' '}<span class="sr-only"
                    >{video.name}</span
                  ></button
                ></td
              ></tr
            >{/each}
        </tbody>
      </table>
    </div>
  {:else}<p>{t(language, 'empty')}</p>{/if}
</section>
