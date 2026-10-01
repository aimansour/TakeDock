<script lang="ts">
  import type { UpdateState } from '../lib/updater';
  import { t } from '../lib/i18n';
  let {
    language,
    update,
    onCheck,
    onInstall,
  }: {
    language: 'en' | 'ar';
    update: UpdateState;
    onCheck: () => void;
    onInstall: () => void;
  } = $props();
</script>

<section class="update-panel">
  <h3>{t(language, 'updates')}</h3>
  <p class="hint">{t(language, 'updateHelp')}</p>
  <button type="button" onclick={onCheck}>{t(language, 'checkUpdates')}</button>
  {#if update.status !== 'idle'}<p>
      {t(language, `update_${update.status}`)}
    </p>{/if}
  {#if update.info}<p><bdi>{update.info.version}</bdi></p>
    {#if update.info.notes}<p class="result-text">
        {update.info.notes}
      </p>{/if}{/if}
  {#if update.status === 'available'}<button
      type="button"
      class="primary"
      onclick={onInstall}>{t(language, 'installUpdate')}</button
    >{/if}
  {#if update.status === 'installing'}<p>
      {(update.bytes / 1048576).toFixed(1)} MB{update.total
        ? ` / ${(update.total / 1048576).toFixed(1)} MB`
        : ''}
    </p>{/if}
  {#if update.message}<p class="result-text">{update.message}</p>{/if}
</section>
