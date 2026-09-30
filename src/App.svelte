<script lang="ts">
  import { onMount } from 'svelte';
  import RecordingPanel from './components/RecordingPanel.svelte';
  import VideosPanel from './components/VideosPanel.svelte';
  import SettingsPanel from './components/SettingsPanel.svelte';
  import { createSession } from './lib/session.svelte';
  import { ipc } from './lib/ipc';
  import { shortcut } from './lib/keyboard';
  import { t } from './lib/i18n';
  import icon from '../assets/icon.svg';
  const app = createSession();
  const data = app.data;
  onMount(() => {
    void app.initialize();
    return app.dispose;
  });
  $effect(() => {
    document.documentElement.lang = data.settings.language;
    document.documentElement.dir =
      data.settings.language === 'ar' ? 'rtl' : 'ltr';
  });
  function keyboard(event: KeyboardEvent) {
    const action = shortcut(event);
    if (!action) return;
    event.preventDefault();
    if (action === 'record')
      app.activate(data.session.predicted === 'idle' ? 'start' : 'stop');
    else if (action === 'pause')
      app.activate(data.session.predicted === 'paused' ? 'resume' : 'pause');
    else {
      data.view = action;
      if (action === 'videos') void app.refresh();
    }
  }
</script>

<svelte:window onkeydown={keyboard} />
<header class="app-header">
  <div>
    <h1>TakeDock</h1>
    <p>
      {t(data.settings.language, 'connection')}: {t(
        data.settings.language,
        data.session.connected ? 'connected' : 'disconnected',
      )}
    </p>
  </div>
  <img class="app-icon" src={icon} alt="" width="64" height="64" />
</header>
<nav class="navigation">
  <button
    class:active={data.view === 'recording'}
    onclick={() => (data.view = 'recording')}
    >{t(data.settings.language, 'recording')}</button
  ><button
    class:active={data.view === 'videos'}
    onclick={() => {
      data.view = 'videos';
      void app.refresh();
    }}>{t(data.settings.language, 'videos')}</button
  ><button
    class:active={data.view === 'settings'}
    onclick={() => (data.view = 'settings')}
    >{t(data.settings.language, 'settings')}</button
  >
</nav>
<main>
  {#if data.view === 'recording'}<RecordingPanel
      state={data.session}
      language={data.settings.language}
      onAction={app.activate}
    />{/if}
  {#if data.view === 'videos'}<VideosPanel
      videos={data.videos}
      connected={data.session.connected}
      loading={data.loading}
      language={data.settings.language}
      onRefresh={() => void app.refresh()}
      onJob={app.job}
    />{/if}
  {#if data.view === 'settings'}<SettingsPanel
      settings={data.settings}
      onSave={(settings) => void app.save(settings)}
      onFolder={ipc.folder}
      onAdb={ipc.adb}
      onReconnect={app.reconnect}
      onError={app.fail}
    />{/if}
  {#if data.result}<section class="panel result-panel">
      <h2>{t(data.settings.language, 'result')}</h2>
      <p class="result-text">{t(data.settings.language, data.result)}</p>
    </section>{/if}
  {#if data.jobs.length}<section class="panel">
      <h2>{t(data.settings.language, 'jobs')}</h2>
      {#each data.jobs as job (job.id)}<article class="job">
          <h3>
            {t(data.settings.language, job.kind)} · {job.completed}/{job.count}
          </h3>
          <p>{t(data.settings.language, job.status)} · <bdi>{job.name}</bdi></p>
          {#if job.status === 'running'}<p>
              {(job.bytes / 1048576).toFixed(1)} MB
            </p>{/if}{#if ['running', 'queued'].includes(job.status)}<button
              onclick={() => app.cancel(job.id)}
              >{t(data.settings.language, 'cancel')}</button
            >{/if}{#if job.results.length}<ul>
              {#each job.results as result}<li>
                  <bdi>{result.name}</bdi>: {t(
                    data.settings.language,
                    result.status,
                  )}{result.message
                    ? ` — ${t(data.settings.language, result.message)}`
                    : ''}
                </li>{/each}
            </ul>{/if}
        </article>{/each}
    </section>{/if}
</main>
