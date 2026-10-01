import { it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import UpdatePanel from './UpdatePanel.svelte';
it('keeps focus on manual check and exposes install only for an offered update', async () => {
  const onCheck = vi.fn(),
    onInstall = vi.fn();
  const { rerender, container } = render(UpdatePanel, {
    language: 'en',
    update: { status: 'idle', info: null, message: '', bytes: 0, total: 0 },
    onCheck,
    onInstall,
  });
  const check = screen.getByRole('button', { name: 'Check for updates' });
  await userEvent.click(check);
  expect(onCheck).toHaveBeenCalledOnce();
  expect(screen.queryByRole('button', { name: 'Install update' })).toBeNull();
  await rerender({
    language: 'en',
    update: {
      status: 'available',
      info: { version: '0.2.0', notes: 'Fixes', date: '' },
      message: '',
      bytes: 0,
      total: 0,
    },
    onCheck,
    onInstall,
  });
  expect(check).toHaveFocus();
  await userEvent.click(screen.getByRole('button', { name: 'Install update' }));
  expect(onInstall).toHaveBeenCalledOnce();
  expect(
    container.querySelector('[aria-live], [role="status"], [role="alert"]'),
  ).toBeNull();
});
