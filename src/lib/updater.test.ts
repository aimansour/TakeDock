import { it, expect, vi } from 'vitest';
import { UpdaterController } from './updater';
it('startup and manual checks offer updates without installing or moving focus', async () => {
  const button = document.createElement('button');
  document.body.append(button);
  button.focus();
  const api = {
    check: vi.fn().mockResolvedValue({
      version: '0.2.0',
      token: 'first',
      notes: 'Fixes',
      date: '',
    }),
    install: vi.fn().mockResolvedValue(undefined),
  };
  const updater = new UpdaterController(api, vi.fn(), vi.fn());
  await updater.checkForUpdate('startup');
  expect(updater.state.status).toBe('available');
  await updater.checkForUpdate('manual');
  expect(api.install).not.toHaveBeenCalled();
  expect(button).toHaveFocus();
  await updater.installUpdate();
  expect(api.install).toHaveBeenCalledTimes(1);
  button.remove();
});
it('no update and network failure never install and preserve focus', async () => {
  const button = document.createElement('button');
  document.body.append(button);
  button.focus();
  const api = { check: vi.fn().mockResolvedValue(null), install: vi.fn() };
  const updater = new UpdaterController(api, vi.fn(), vi.fn());
  await updater.checkForUpdate('manual');
  expect(updater.state.status).toBe('current');
  api.check.mockRejectedValue(new Error('offline'));
  await updater.checkForUpdate('startup');
  expect(updater.state.status).toBe('error');
  expect(button).toHaveFocus();
  expect(api.install).not.toHaveBeenCalled();
  button.remove();
});
it('does not install before a checked offer or issue duplicate concurrent checks', async () => {
  let resolve!: (value: null) => void;
  const api = {
    check: vi.fn(() => new Promise<null>((callback) => (resolve = callback))),
    install: vi.fn(),
  };
  const updater = new UpdaterController(api, vi.fn(), vi.fn());
  await updater.installUpdate();
  expect(api.install).not.toHaveBeenCalled();
  const first = updater.checkForUpdate('startup');
  const second = updater.checkForUpdate('manual');
  expect(api.check).toHaveBeenCalledTimes(1);
  resolve(null);
  await Promise.all([first, second]);
});
it('installs only the immutable offer explicitly displayed in this window', async () => {
  const offer = { version: '0.2.0', token: 'offer-X', notes: '', date: '' };
  const api = {
    check: vi.fn().mockResolvedValue(offer),
    install: vi.fn().mockResolvedValue(undefined),
  };
  const updater = new UpdaterController(api, vi.fn(), vi.fn());
  await updater.checkForUpdate('manual');
  await updater.installUpdate();
  expect(api.install).toHaveBeenCalledWith('offer-X');
});
