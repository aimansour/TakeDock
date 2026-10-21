import { render, screen, within, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { expect, it, vi } from 'vitest';
import FileResultsPanel from './FileResultsPanel.svelte';
import type { FileNotice } from '../lib/types';
const notice = (
  id = 'one',
  outcome: FileNotice['outcome'] = 'completed',
): FileNotice => ({
  key: { generation: 7, owner: 'job', operation_id: id },
  revision: 1,
  kind: 'rename',
  outcome,
  code: outcome,
  warnings: [],
  name: `${id}.mp4`,
  count: 1,
  completed: outcome === 'completed' ? 1 : 0,
  results: [
    {
      name: `${id}.mp4`,
      status: outcome,
      message: '',
      requested_name: 'requested.mp4',
      destination_name: outcome === 'completed' ? 'renamed.mp4' : null,
    },
  ],
  expires_at_ms: outcome === 'completed' ? 10000 : null,
});
const props = (notices = [notice()]) => ({
  notices,
  history: notices,
  language: 'en' as const,
  onHide: vi.fn(async () => {}),
});

it('keeps native history closed with separate source and committed destination text', () => {
  const { container } = render(FileResultsPanel, props());
  expect(container.querySelector('details')?.open).toBe(false);
  expect(screen.getByText('Operation history')).toBeInTheDocument();
  const card = container.querySelector('.file-result-notice')!;
  expect(
    within(card as HTMLElement).getByText('renamed.mp4'),
  ).toBeInTheDocument();
  expect(within(card as HTMLElement).queryByText('requested.mp4')).toBeNull();
  expect(
    container.querySelector('[aria-live], [role="alert"], [role="status"]'),
  ).toBeNull();
});
it('failure_survives_later_success', async () => {
  const failed = notice('failed', 'failed');
  const { rerender, container } = render(
    FileResultsPanel,
    props([notice(), failed]),
  );
  await rerender({ notices: [failed] });
  expect(container.querySelectorAll('.file-result-notice')).toHaveLength(1);
  expect(
    screen.getByRole('button', { name: /Hide result.*failed\.mp4/ }),
  ).toBeInTheDocument();
  const card = container.querySelector('.file-result-notice') as HTMLElement;
  expect(
    within(card).getByText('requested.mp4').closest('p'),
  ).toHaveTextContent('Requested name');
  expect(card.querySelector('[data-committed-name]')).toBeNull();
});
it('remote_ack_and_passive_expiry_keep_focused_card', async () => {
  const { rerender } = render(FileResultsPanel, props());
  const button = screen.getByRole('button', { name: /Hide result/ });
  button.focus();
  await rerender({ notices: [] });
  expect(screen.getByRole('button', { name: /Hide result/ })).toBe(button);
  expect(button).toHaveFocus();
  screen.getByText('Operation history').focus();
  await tick();
  await tick();
  expect(screen.queryByRole('button', { name: /Hide result/ })).toBeNull();
});
it('local hide returns focus once to history after its own acknowledgement', async () => {
  const user = userEvent.setup();
  let acknowledge!: () => void;
  const onHide = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        acknowledge = resolve;
      }),
  );
  const { rerender } = render(FileResultsPanel, { ...props(), onHide });
  await user.click(screen.getByRole('button', { name: /Hide result/ }));
  await rerender({ notices: [] });
  acknowledge();
  await tick();
  await tick();
  await waitFor(() =>
    expect(screen.getByText('Operation history')).toHaveFocus(),
  );
  expect(screen.queryByRole('button', { name: /Hide result/ })).toBeNull();
});
it('local_hide_does_not_reclaim_moved_focus', async () => {
  const user = userEvent.setup();
  let acknowledge!: () => void;
  const { rerender } = render(FileResultsPanel, {
    ...props(),
    onHide: () =>
      new Promise<void>((resolve) => {
        acknowledge = resolve;
      }),
  });
  await user.click(screen.getByRole('button', { name: /Hide result/ }));
  const elsewhere = document.createElement('button');
  elsewhere.textContent = 'Continue recording';
  document.body.append(elsewhere);
  elsewhere.focus();
  await rerender({ notices: [] });
  acknowledge();
  await tick();
  expect(elsewhere).toHaveFocus();
  elsewhere.remove();
});
it('a newer warning or rejected hide does not move focus', async () => {
  const user = userEvent.setup();
  let acknowledge!: () => void;
  const { rerender } = render(FileResultsPanel, {
    ...props(),
    onHide: () =>
      new Promise<void>((resolve) => {
        acknowledge = resolve;
      }),
  });
  const button = screen.getByRole('button', { name: /Hide result/ });
  await user.click(button);
  await rerender({
    notices: [
      {
        ...notice(),
        revision: 2,
        outcome: 'warning',
        warnings: ['rename_refresh_pending'],
        expires_at_ms: null,
      },
    ],
  });
  acknowledge();
  await tick();
  expect(button).toHaveFocus();
  await rerender({
    onHide: async () => {
      throw new Error('offline');
    },
  });
  await user.click(button);
  expect(button).toHaveFocus();
});
it('open_history_pins_displayed_records_past_twenty', async () => {
  const user = userEvent.setup();
  const initial = Array.from({ length: 20 }, (_, i) => notice(`old-${i}`));
  const { rerender, container } = render(FileResultsPanel, {
    ...props([]),
    history: initial,
  });
  await user.click(screen.getByText('Operation history'));
  const next = Array.from({ length: 20 }, (_, i) => notice(`new-${i}`));
  await rerender({ history: next });
  expect(container.querySelectorAll('details article')).toHaveLength(40);
  await user.click(screen.getByText('Operation history'));
  await tick();
  expect(container.querySelectorAll('details article')).toHaveLength(20);
});
it('renders Arabic native labels and empty history', () => {
  render(FileResultsPanel, { ...props([]), language: 'ar' });
  expect(screen.getByText('سجل العمليات')).toBeInTheDocument();
  expect(
    screen.getByText('لا توجد عمليات ملفات مكتملة بعد.'),
  ).toBeInTheDocument();
});
it('qualifies Arabic hide controls with the operation and filename', () => {
  render(FileResultsPanel, { ...props(), language: 'ar' });
  expect(
    screen.getByRole('button', { name: /إخفاء النتيجة.*one\.mp4/ }),
  ).toHaveAccessibleName(/إعادة تسمية الفيديو/);
});
