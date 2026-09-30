<script lang="ts">
  import type { Settings } from '../lib/types';
  import { defaultSettings } from '../lib/types';
  import { t } from '../lib/i18n';
  import type { Snippet } from 'svelte';
  let {
    settings,
    onSave,
    onFolder,
    onAdb,
    onReconnect,
    onError = () => {},
    children,
  }: {
    settings: Settings;
    onSave: (settings: Settings) => void;
    onFolder: () => Promise<string | null>;
    onAdb: () => Promise<string | null>;
    onReconnect: () => void;
    onError?: (message: string) => void;
    children?: Snippet;
  } = $props();
  let draft = $state<Settings>({ ...defaultSettings });
  let seconds = $state(10);
  $effect(() => {
    draft = { ...settings };
    seconds = settings.verification_ms / 1000;
  });
  async function pick(kind: 'folder' | 'adb') {
    try {
      const path = await (kind === 'folder' ? onFolder() : onAdb());
      if (path) {
        if (kind === 'folder') draft.destination = path;
        else draft.adb_path = path;
      }
    } catch (error) {
      onError(String(error));
    }
  }
</script>

<section
  class="panel settings-panel"
  dir={settings.language === 'ar' ? 'rtl' : 'ltr'}
>
  <h2>{t(settings.language, 'settings')}</h2>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      onSave({ ...draft, verification_ms: seconds * 1000 });
    }}
  >
    <label for="language">{t(settings.language, 'language')}</label><select
      id="language"
      bind:value={draft.language}
      ><option value="en">English</option><option value="ar">العربية</option
      ></select
    >
    <label for="destination">{t(settings.language, 'destination')}</label>
    <div class="input-row">
      <input
        id="destination"
        bind:value={draft.destination}
        dir="auto"
      /><button type="button" onclick={() => void pick('folder')}
        >{t(settings.language, 'chooseFolder')}</button
      >
    </div>
    <label for="adb-path">{t(settings.language, 'adbPath')}</label>
    <div class="input-row">
      <input id="adb-path" bind:value={draft.adb_path} dir="auto" /><button
        type="button"
        onclick={() => void pick('adb')}
        >{t(settings.language, 'chooseAdb')}</button
      >
    </div>
    <label class="check-label"
      ><input type="checkbox" bind:checked={draft.success_sound} />{t(
        settings.language,
        'successSound',
      )}</label
    >
    <p class="hint">{t(settings.language, 'failureSoundHelp')}</p>
    <label for="verification">{t(settings.language, 'verification')}</label
    ><input
      id="verification"
      type="number"
      bind:value={seconds}
      min="2"
      max="120"
      step="1"
      required
    />
    <div class="toolbar">
      <button class="primary" type="submit"
        >{t(settings.language, 'saveSettings')}</button
      ><button type="button" onclick={onReconnect}
        >{t(settings.language, 'reconnect')}</button
      >
    </div>
  </form>
  <p class="hint">{t(settings.language, 'settingsHelp')}</p>
  {#if children}{@render children()}{/if}
</section>
