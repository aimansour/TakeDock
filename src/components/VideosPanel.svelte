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
  }: {
    videos: Video[];
    onJob: (kind: JobKind, videos: Video[], stem?: string) => void;
    language?: 'en' | 'ar';
    connected?: boolean;
    onRefresh: () => void;
    loading?: boolean;
  } = $props();
  let selected = $state<string[]>([]);
  let renaming = $state(false);
  let stem = $state('');
  let deleting = $state<Video[]>([]);
  let renameInput = $state<HTMLInputElement>();
  let confirmation = $state<HTMLButtonElement>();
  let toolbar = $state<HTMLButtonElement>();
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
  {#if !connected}<p>{t(language, 'noDevice')}</p>{/if}
  {#if loading}<p>{t(language, 'loading')}</p>{/if}
  {#if connected && videos.some((video) => video.ready)}
    <div class="toolbar">
      <button
        onclick={() =>
          (selected = videos.filter((video) => video.ready).map(videoKey))}
        >{t(language, 'selectAll')}</button
      >{#if selected.length}<button onclick={() => (selected = [])}
          >{t(language, 'clear')}</button
        >{/if}
    </div>
  {/if}
  {#if connected && chosen.length}
    <p>{t(language, 'selected')}: {chosen.length}</p>
    <div class="toolbar">
      <button onclick={() => onJob('copy', chosen)}
        >{t(language, 'copy')}</button
      ><button onclick={() => onJob('move', chosen)}
        >{t(language, 'move')}</button
      >
      <button
        class="danger"
        onclick={() => {
          deleting = [...chosen];
          void tick().then(() => confirmation?.focus());
        }}>{t(language, 'delete')}</button
      >
      {#if chosen.length === 1}<button onclick={openRename}
          >{t(language, 'rename')}</button
        >{/if}
    </div>
  {/if}
  {#if renaming && chosen.length === 1}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        onJob('rename', [...chosen], stem);
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
            onJob('delete', deleting);
            closeForm();
          }}>{t(language, 'confirmDelete')}</button
        ><button onclick={closeForm}>{t(language, 'cancel')}</button>
      </div>
    </fieldset>
  {/if}
  {#if videos.length}
    <div class="table-scroll">
      <table>
        <thead
          ><tr
            ><th scope="col">{t(language, 'select')}</th><th scope="col"
              >{t(language, 'name')}</th
            ><th scope="col">{t(language, 'size')}</th><th scope="col"
              >{t(language, 'modified')}</th
            ><th scope="col">{t(language, 'duration')}</th><th scope="col"
              >{t(language, 'state')}</th
            ></tr
          ></thead
        ><tbody>
          {#each videos as video (videoKey(video))}<tr
              ><td
                >{#if video.ready && connected}<label
                    ><input
                      type="checkbox"
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
                >{t(language, video.ready ? 'complete' : 'protected')}</td
              ></tr
            >{/each}
        </tbody>
      </table>
    </div>
  {:else}<p>{t(language, 'empty')}</p>{/if}
</section>
