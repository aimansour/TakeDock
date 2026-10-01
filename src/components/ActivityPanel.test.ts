import { render, screen } from '@testing-library/svelte';
import { it, expect } from 'vitest';
import ActivityPanel from './ActivityPanel.svelte';
import type { JobEvent } from '../lib/types';
const job: JobEvent = {
  id: 'one',
  generation: 3,
  kind: 'copy',
  status: 'running',
  phase: 'copying',
  name: 'one.mp4',
  bytes: 30,
  total: 100,
  completed: 0,
  count: 2,
  message: '',
  results: [],
};
it('shows indeterminate listing progress while keeping completed results available afterwards', async () => {
  const { rerender } = render(ActivityPanel, {
    jobs: [{ ...job, status: 'completed' }],
    loading: true,
    language: 'en',
  });
  expect(
    screen.getByRole('progressbar', { name: 'Activity progress' }),
  ).not.toHaveAttribute('value');
  expect(screen.getByText('Loading videos')).toBeInTheDocument();
  await rerender({ loading: false });
  expect(
    screen.getByRole('progressbar', { name: 'Activity progress' }),
  ).toHaveAttribute('value', '100');
});
it('exposes native changing progress without live speech and keeps verification distinct', async () => {
  const { rerender, container } = render(ActivityPanel, {
    jobs: [job],
    loading: false,
    language: 'en',
  });
  let progress = screen.getByRole('progressbar', { name: 'Activity progress' });
  expect(progress).toHaveAttribute('value', '30');
  expect(progress).toHaveAttribute('max', '100');
  expect(screen.getByText('Copying video')).toBeInTheDocument();
  await rerender({ jobs: [{ ...job, phase: 'verifying', bytes: 100 }] });
  progress = screen.getByRole('progressbar', { name: 'Activity progress' });
  expect(progress).not.toHaveAttribute('value');
  expect(screen.getByText('Verifying copied video')).toBeInTheDocument();
  await rerender({ jobs: [{ ...job, status: 'completed', completed: 2 }] });
  progress = screen.getByRole('progressbar', { name: 'Activity progress' });
  expect(progress).toHaveAttribute('value', '100');
  expect(screen.getByText('Completed')).toBeInTheDocument();
  expect(
    container.querySelector('[aria-live], [role="alert"], [role="status"]'),
  ).toBeNull();
});
it('keeps active processing visible before newer queued jobs and reports failures', async () => {
  const { rerender } = render(ActivityPanel, {
    jobs: [{ ...job, id: 'two', status: 'queued' }, job],
    loading: false,
    language: 'en',
  });
  expect(screen.getByText('Copying video')).toBeInTheDocument();
  await rerender({
    jobs: [{ ...job, status: 'failed', message: 'copy_hash_mismatch' }],
  });
  expect(screen.getByText('Failed')).toBeInTheDocument();
  expect(screen.getByText(/The copied file did not match/)).toBeInTheDocument();
});
