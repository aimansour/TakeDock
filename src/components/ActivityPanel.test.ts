import { render, screen } from '@testing-library/svelte';
import { it, expect } from 'vitest';
import ActivityPanel from './ActivityPanel.svelte';
import type { JobEvent, FileNotice } from '../lib/types';
const notice = (outcome: FileNotice['outcome'] = 'completed'): FileNotice => ({
  key: { generation: 3, owner: 'job', operation_id: 'one' },
  revision: 1,
  kind: 'copy',
  outcome,
  code: outcome === 'failed' ? 'copy_hash_mismatch' : outcome,
  warnings: [],
  name: 'one.mp4',
  completed: outcome === 'completed' ? 2 : 0,
  count: 2,
  results: [],
  expires_at_ms: outcome === 'completed' ? 10000 : null,
});
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
    notices: [notice()],
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
  await rerender({
    jobs: [{ ...job, status: 'completed', completed: 2 }],
    notices: [notice()],
  });
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
    notices: [notice('failed')],
  });
  expect(screen.getByText('Failed')).toBeInTheDocument();
  expect(screen.getByText(/The copied file did not match/)).toBeInTheDocument();
});
it('retires terminal progress and keeps all active jobs cancellable', async () => {
  const { rerender } = render(ActivityPanel, {
    jobs: [{ ...job, status: 'completed' }],
    notices: [],
    loading: false,
    language: 'en',
  });
  expect(screen.getByRole('progressbar')).toHaveAttribute('value', '0');
  expect(screen.getByText('Ready')).toBeInTheDocument();
  await rerender({
    jobs: [job, { ...job, id: 'queued', status: 'queued' }],
    notices: [notice('failed')],
  });
  expect(screen.getAllByRole('button', { name: /Cancel/ })).toHaveLength(2);
});
it('keeps cancellation explicit when a guarded deletion retains the file', async () => {
  const retained: FileNotice = {
    ...notice('retained'),
    kind: 'delete',
    code: 'discard_capture_changed',
  };
  const { rerender } = render(ActivityPanel, {
    jobs: [],
    notices: [notice(), retained],
    loading: false,
    language: 'en',
  });
  expect(screen.getByRole('progressbar')).toHaveAttribute('value', '100');
  await rerender({ notices: [retained] });
  expect(screen.getByRole('progressbar')).toHaveAttribute('value', '0');
  expect(screen.getByText('Cancelled')).toBeInTheDocument();
  expect(screen.getByText('Retained on phone')).toBeInTheDocument();
  await rerender({
    notices: [{ ...retained, code: 'discard_inspection_failed' }],
  });
  expect(screen.queryByText('Cancelled')).toBeNull();
});
