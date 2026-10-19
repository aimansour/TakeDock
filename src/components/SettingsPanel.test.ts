import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { it, expect, vi } from 'vitest';
import SettingsPanel from './SettingsPanel.svelte';
import { defaultSettings } from '../lib/types';
it('defaults readiness independently of optional success feedback', () => {
  const { container } = render(SettingsPanel, {
    settings: defaultSettings,
    onSave: vi.fn(),
    onFolder: vi.fn(),
    onAdb: vi.fn(),
    onReconnect: vi.fn(),
  });
  expect(
    screen.getByRole('checkbox', { name: 'Play the ready-to-speak cue' }),
  ).toBeChecked();
  expect(
    screen.getByRole('checkbox', { name: 'Play a sound on success' }),
  ).not.toBeChecked();
  const delay = screen.getByLabelText(
    'Speech-readiness delay after confirmation (milliseconds)',
  );
  expect(delay).toHaveValue(800);
  expect(delay).toHaveAttribute('id', 'ready-delay');
  expect(delay).toHaveAttribute('min', '500');
  expect(delay).toHaveAttribute('max', '5000');
  expect(delay).toHaveAttribute('step', '50');
  expect(delay).toBeRequired();
  const help = container.querySelector(
    `#${delay.getAttribute('aria-describedby')}`,
  );
  expect(help).toHaveTextContent('after confirmation');
  expect(help).toHaveTextContent('next Start');
  expect(help).toHaveTextContent('first words');
});
it('saves readiness with the keyboard independently and shares the Arabic labels', async () => {
  const user = userEvent.setup();
  const onSave = vi.fn();
  const { rerender, container } = render(SettingsPanel, {
    settings: defaultSettings,
    onSave,
    onFolder: vi.fn(),
    onAdb: vi.fn(),
    onReconnect: vi.fn(),
  });
  const cue = screen.getByRole('checkbox', {
    name: 'Play the ready-to-speak cue',
  });
  cue.focus();
  await user.keyboard(' ');
  const delay = screen.getByLabelText(
    'Speech-readiness delay after confirmation (milliseconds)',
  );
  await user.clear(delay);
  await user.type(delay, '1250');
  await user.selectOptions(screen.getByLabelText('Language'), 'ar');
  await user.click(screen.getByRole('button', { name: 'Save settings' }));
  expect(onSave).toHaveBeenCalledOnce();
  const saved = onSave.mock.calls[0][0];
  expect(saved).toMatchObject({
    language: 'ar',
    ready_sound: false,
    ready_delay_ms: 1250,
    success_sound: false,
  });
  await rerender({ settings: saved });
  expect(
    screen.getByRole('checkbox', { name: 'تشغيل صفارة الجاهزية للكلام' }),
  ).not.toBeChecked();
  expect(
    screen.getByLabelText('مهلة الجاهزية للكلام بعد التأكيد (مللي ثانية)'),
  ).toHaveValue(1250);
  expect(container.querySelector('section')).toHaveAttribute('dir', 'rtl');
  expect(
    container.querySelector('[aria-live], [role="alert"], [role="status"]'),
  ).toBeNull();
});
it.each(['', '499', '501', '525', '5001'])(
  'prevents saving an invalid readiness delay %s',
  async (value) => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(SettingsPanel, {
      settings: defaultSettings,
      onSave,
      onFolder: vi.fn(),
      onAdb: vi.fn(),
      onReconnect: vi.fn(),
    });
    const delay = screen.getByLabelText(
      'Speech-readiness delay after confirmation (milliseconds)',
    );
    await user.clear(delay);
    if (value) await user.type(delay, value);
    await user.click(screen.getByRole('button', { name: 'Save settings' }));
    expect(delay).toBeInvalid();
    expect(onSave).not.toHaveBeenCalled();
  },
);
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
