<script lang="ts">
  import type { JobEvent, FileNotice } from '../lib/types';
  import { t } from '../lib/i18n';
  let {
    jobs = [],
    loading = false,
    language = 'en',
    notices = [],
    onCancel = () => {},
  }: {
    jobs: JobEvent[];
    loading: boolean;
    language: 'en' | 'ar';
    notices?: FileNotice[];
    onCancel?: (id: string) => void;
  } = $props();
  const active = $derived(
    jobs.filter((job) => ['queued', 'running'].includes(job.status)),
  );
  const job = $derived(
    jobs.find((job) => job.status === 'running') ??
      jobs.find((job) => job.status === 'queued'),
  );
  const notice = $derived(!job && !loading ? notices[0] : undefined);
  const value = $derived(
    notice
      ? ['completed', 'warning'].includes(notice.outcome) &&
        notice.completed === notice.count
        ? 100
        : 0
      : !job && !loading
        ? 0
        : undefined,
  );
</script>

<section class="panel activity-panel">
  <h2>{t(language, 'activity')}</h2>
  <label for="activity-progress">{t(language, 'activityProgress')}</label>
  {#if value === undefined}<progress id="activity-progress" max="100"
    ></progress>
  {:else}<progress id="activity-progress" max="100" {value}></progress>{/if}
  {#if job}
    <p>
      {t(
        language,
        job.status === 'running' ? (job.phase ?? 'running') : job.status,
      )}
    </p>
    <p>
      {t(language, job.kind)} · <bdi>{job.name}</bdi> · {job.completed}/{job.count}
      {t(language, 'count')}
    </p>
    {#if job.message}<p>{t(language, job.message)}</p>{/if}
  {:else if notice}
    <p>{t(language, notice.outcome)}</p>
    {#if notice.outcome === 'retained' && ['discard_capture_changed', 'discard_cancelled'].includes(notice.code)}
      <p>{t(language, 'cancelled')}</p>
    {/if}
    <p>
      {t(language, notice.kind)} · <bdi>{notice.name}</bdi> · {notice.completed}/{notice.count}
      {t(language, 'count')}
    </p>
    {#if notice.code !== notice.outcome}<p>{t(language, notice.code)}</p>{/if}
    {#each notice.warnings as warning}<p>{t(language, warning)}</p>{/each}
  {:else}<p>{t(language, loading ? 'loadingVideos' : 'ready')}</p>{/if}
  {#if loading && job}<p>{t(language, 'loadingVideos')}</p>{/if}
  {#each active as pending (pending.id)}
    <article class="job">
      <p>
        {t(language, pending.kind)} · <bdi>{pending.name}</bdi> · {t(
          language,
          pending.status,
        )} · {pending.completed}/{pending.count}
      </p>
      <button onclick={() => onCancel(pending.id)}
        >{t(language, 'cancel')} · {t(language, pending.kind)} ·
        <bdi>{pending.name}</bdi></button
      >
    </article>
  {/each}
</section>
