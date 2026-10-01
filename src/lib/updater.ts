export interface UpdateInfo {
  version: string;
  notes: string;
  date: string;
}
export interface UpdateState {
  status:
    | 'idle'
    | 'checking'
    | 'available'
    | 'current'
    | 'installing'
    | 'installed'
    | 'error';
  info: UpdateInfo | null;
  message: string;
  bytes: number;
  total: number;
}
export interface UpdateApi {
  check: () => Promise<UpdateInfo | null>;
  install: () => Promise<void>;
}
export class UpdaterController {
  state: UpdateState = {
    status: 'idle',
    info: null,
    message: '',
    bytes: 0,
    total: 0,
  };
  private checking: Promise<void> | null = null;
  constructor(
    private api: UpdateApi,
    private changed: (state: UpdateState) => void,
    private error: (message: string) => void,
  ) {}
  private set(patch: Partial<UpdateState>) {
    this.state = { ...this.state, ...patch };
    this.changed(this.state);
  }
  checkForUpdate(source: 'startup' | 'manual'): Promise<void> {
    void source;
    if (this.checking) return this.checking;
    if (this.state.status === 'installing') return Promise.resolve();
    this.set({ status: 'checking', info: null, message: '' });
    this.checking = (async () => {
      try {
        const info = await this.api.check();
        this.set({ status: info ? 'available' : 'current', info });
      } catch (error) {
        this.set({ status: 'error', message: String(error) });
        this.error(String(error));
      } finally {
        this.checking = null;
      }
    })();
    return this.checking;
  }
  async installUpdate(): Promise<void> {
    if (this.state.status !== 'available' || !this.state.info) return;
    this.set({ status: 'installing', bytes: 0, total: 0, message: '' });
    try {
      await this.api.install();
      this.set({ status: 'installed' });
    } catch (error) {
      this.set({ status: 'error', message: String(error) });
      this.error(String(error));
    }
  }
  progress(bytes: number, total: number) {
    if (this.state.status === 'installing') this.set({ bytes, total });
  }
}
