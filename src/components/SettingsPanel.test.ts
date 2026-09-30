import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { it, expect, vi } from 'vitest';
import SettingsPanel from './SettingsPanel.svelte';
import { defaultSettings } from '../lib/types';
it('saves language and optional success sound through native labeled inputs', async () => {
  const user = userEvent.setup();
  const onSave = vi.fn();
  const { container, rerender } = render(SettingsPanel, {
    settings: defaultSettings,
    onSave,
    onFolder: vi.fn(),
    onAdb: vi.fn(),
    onReconnect: vi.fn(),
  });
  expect(
    screen.getByRole('checkbox', { name: 'Play a sound on success' }),
  ).not.toBeChecked();
  await user.selectOptions(screen.getByLabelText('Language'), 'ar');
  await user.click(
    screen.getByRole('checkbox', { name: 'Play a sound on success' }),
  );
  await user.click(screen.getByRole('button', { name: 'Save settings' }));
  expect(onSave.mock.calls[0][0]).toMatchObject({
    language: 'ar',
    success_sound: true,
  });
  await rerender({ settings: { ...defaultSettings, language: 'ar' } });
  expect(container.querySelector('section')).toHaveAttribute('dir', 'rtl');
  expect(
    screen.getByRole('button', { name: 'حفظ الإعدادات' }),
  ).toBeInTheDocument();
  expect(
    container.querySelector('[aria-live], [role="alert"], [role="status"]'),
  ).toBeNull();
});
