<script lang="ts">
  import { t } from '../lib/i18n';
  let {
    name,
    language,
    onRename,
    onNew,
  }: {
    name: string;
    language: 'en' | 'ar';
    onRename: (name: string) => void;
    onNew: () => void;
  } = $props();
  let draft = $state('');
  $effect(() => {
    draft = name;
  });
</script>

<fieldset>
  <legend>{t(language, 'windowName')}</legend>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      onRename(draft);
    }}
  >
    <label for="window-name">{t(language, 'windowName')}</label>
    <input
      id="window-name"
      bind:value={draft}
      maxlength="80"
      required
      dir="auto"
    />
    <div class="toolbar">
      <button type="submit">{t(language, 'renameWindow')}</button><button
        type="button"
        onclick={onNew}>{t(language, 'newWindow')}</button
      >
    </div>
  </form>
  <p class="hint">{t(language, 'windowHelp')}</p>
</fieldset>
