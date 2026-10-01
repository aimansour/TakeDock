<script lang="ts">
  import type { JobEvent } from '../lib/types';
  import { t } from '../lib/i18n';
  let {
    jobs = [],
    loading = false,
    language = 'en',
  }: { jobs: JobEvent[]; loading: boolean; language: 'en' | 'ar' } = $props();
  const job = $derived(
    jobs.find((job) => job.status === 'running') ??
      jobs.find((job) => job.status === 'queued') ??
      (loading ? undefined : jobs[0]),
  );
  const transferring = $derived(
    job?.status === 'running' && job.phase === 'copying' && job.total > 0,
  );
  const value = $derived(
    job?.status === 'completed'
      ? 100
      : transferring && job
        ? Math.min(100, (job.bytes / job.total) * 100)
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
    {#if transferring}<p>
        {(job.bytes / 1048576).toLocaleString(language, {
          maximumFractionDigits: 1,
        })} / {(job.total / 1048576).toLocaleString(language, {
          maximumFractionDigits: 1,
        })} MB
      </p>{/if}
    {#if job.message}<p>{t(language, job.message)}</p>{/if}
  {:else}<p>{t(language, loading ? 'loadingVideos' : 'ready')}</p>{/if}
  {#if loading && job}<p>{t(language, 'loadingVideos')}</p>{/if}
</section>
