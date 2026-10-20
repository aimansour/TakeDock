<script lang="ts">
  import type { SessionState, RecordingAction } from '../lib/types';
  import { tick } from 'svelte';
  import { t } from '../lib/i18n';
  let {
    state: session,
    onAction,
    onDiscard,
    language = 'en',
  }: {
    state: SessionState;
    onAction: (action: RecordingAction) => void;
    onDiscard: () => void;
    language?: 'en' | 'ar';
  } = $props();
  let primary = $state<HTMLButtonElement>();
  let secondary = $state<HTMLButtonElement>();
  let heading = $state<HTMLHeadingElement>();
  let discardCheckbox = $state<HTMLInputElement>();
  let deleteTake = $state(false);
  let confirmedTake = $state('');
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
  let canDiscard = $derived(
    eligible &&
      recording &&
      session.discard.eligible &&
      session.discard.take?.generation === session.generation,
  );
  $effect.pre(() => {
    const take = session.discard.take;
    const key = take
      ? `${session.generation}:${take.start_sequence}:${take.id}`
      : '';
    if (key !== confirmedTake || !canDiscard) {
      deleteTake = false;
      confirmedTake = key;
    }
    const focused = document.activeElement;
    if (
      !eligible &&
      (focused === primary ||
        focused === secondary ||
        focused === discardCheckbox)
    ) {
      void tick().then(() => heading?.focus());
    } else if (!canDiscard && discardCheckbox && focused === discardCheckbox) {
      void tick().then(() => primary?.focus());
    } else if (!recording && secondary && focused === secondary) {
      void tick().then(() => primary?.focus());
    }
  });
</script>

<section class="panel recording-panel" dir={language === 'ar' ? 'rtl' : 'ltr'}>
  <h2 bind:this={heading} tabindex="-1">{t(language, 'recording')}</h2>
  <p class="state-line">
    {t(
      language,
      session.predicted === 'recording' ? 'recordingState' : session.predicted,
    )}
  </p>
  {#if session.speech_readiness !== 'inactive'}
    <p id="speech-readiness" data-readiness={session.speech_readiness}>
      {t(
        language,
        session.speech_readiness === 'preparing'
          ? 'preparingToSpeak'
          : 'readyToSpeak',
      )}
    </p>
  {/if}
  {#if eligible}
    <div class="capture-actions">
      <button
        class="primary capture-primary"
        bind:this={primary}
        onclick={() => {
          if (recording && deleteTake && canDiscard) onDiscard();
          else onAction(recording ? 'stop' : 'start');
        }}
        >{t(
          language,
          recording
            ? deleteTake && canDiscard
              ? 'stopAndDelete'
              : 'stop'
            : 'start',
        )}</button
      >
      {#if canDiscard}<label class="discard-choice"
          ><input
            name="discard-take"
            type="checkbox"
            bind:this={discardCheckbox}
            bind:checked={deleteTake}
          />{t(language, 'deleteTake')}</label
        >{/if}
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
  {#if !['unavailable', 'tracking'].includes(session.discard.phase)}
    <p data-discard-phase={session.discard.phase}>
      {t(language, `discard_${session.discard.phase}`)}
    </p>
    {#if session.discard.reason && session.discard.reason !== `discard_${session.discard.phase}`}<p
      >
        {t(language, session.discard.reason)}
      </p>{/if}
  {:else if recording && !canDiscard && session.discard.reason}<p>
      {t(language, session.discard.reason)}
    </p>{/if}
  {#if canDiscard}<p class="hint">{t(language, 'discardShortcutHelp')}</p>
    <p class="hint">{t(language, 'discardAssumptionHelp')}</p>{/if}
  <p class="hint">{t(language, 'captureHelp')}</p>
  <p class="hint" dir="auto">{t(language, 'recordingKeys')}</p>
</section>
