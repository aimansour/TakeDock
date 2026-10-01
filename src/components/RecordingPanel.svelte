<script lang="ts">
  import type { SessionState, RecordingAction } from '../lib/types';
  import { tick } from 'svelte';
  import { t } from '../lib/i18n';
  let {
    state: session,
    onAction,
    language = 'en',
  }: {
    state: SessionState;
    onAction: (action: RecordingAction) => void;
    language?: 'en' | 'ar';
  } = $props();
  let primary = $state<HTMLButtonElement>();
  let secondary = $state<HTMLButtonElement>();
  let heading = $state<HTMLHeadingElement>();
  let eligible = $derived(
    session.connected &&
      session.observer_ready &&
      session.foreground &&
      session.video_mode &&
      session.predicted !== 'unknown',
  );
  let recording = $derived(
    session.predicted === 'recording' || session.predicted === 'paused',
  );
  $effect.pre(() => {
    const focused = document.activeElement;
    if (!eligible && (focused === primary || focused === secondary)) {
      void tick().then(() => heading?.focus());
    } else if (!recording && secondary && focused === secondary) {
      void tick().then(() => primary?.focus());
    }
  });
</script>

<section class="panel recording-panel">
  <h2 bind:this={heading} tabindex="-1">{t(language, 'recording')}</h2>
  <p class="state-line">
    {t(
      language,
      session.predicted === 'recording' ? 'recordingState' : session.predicted,
    )}
  </p>
  {#if eligible}
    <div class="capture-actions">
      <button
        class="primary capture-primary"
        bind:this={primary}
        onclick={() => onAction(recording ? 'stop' : 'start')}
        >{t(language, recording ? 'stop' : 'start')}</button
      >
      {#if recording}<button
          class="capture-secondary"
          bind:this={secondary}
          onclick={() =>
            onAction(session.predicted === 'paused' ? 'resume' : 'pause')}
          >{t(
            language,
            session.predicted === 'paused' ? 'resume' : 'pause',
          )}</button
        >{/if}
    </div>
  {:else}<p>{t(language, session.condition)}</p>{/if}
  <p class="hint">{t(language, 'captureHelp')}</p>
  <p class="hint" dir="auto">{t(language, 'recordingKeys')}</p>
</section>
