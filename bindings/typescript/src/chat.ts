import * as ffi from "@arut/ffi";
import type { StreamCancellable } from "@boltffi/runtime";
import type { ObservableStore } from "./observable";

// BoltFFI v0.30.1 exposes this from its loaders but omits it from the declarations.
const initialized = (ffi as typeof ffi & { initialized: Promise<void> }).initialized;

export type ChatSnapshot = ffi.ChatState & {
  chatId: string;
  draft: string;
  composerStatus: ffi.ComposerStatus;
  composerError: string;
};
export const ChatRole = ffi.ChatRole;
export const ChatStatus = ffi.ChatStatus;
export const ComposerStatus = ffi.ComposerStatus;
export type ChatSummary = ffi.ChatSummary;

export async function createChatStore(): Promise<ChatStore> {
  await initialized;
  return new ChatStore();
}

export async function createChatController(): Promise<ChatStore> {
  return createChatStore();
}

export type ChatController = ChatStore;

export class ChatStore implements ObservableStore<ChatSnapshot> {
  private readonly session = ffi.createProductSession("", "local-demo");
  private readonly listeners = new Set<() => void>();
  private client!: ffi.ChatHandle;
  private composer!: ffi.ComposerHandle;
  private chatStream!: StreamCancellable<bigint>;
  private composerStream!: StreamCancellable<bigint>;
  private snapshot!: ChatSnapshot;
  private desiredDraft = "";
  private replacing = false;
  private flushPromise?: Promise<void>;
  private disposed = false;
  private epoch = 0;
  private pollTimer?: ReturnType<typeof setTimeout>;

  constructor() {
    this.bind(this.session.chat());
  }

  getSnapshot = (): ChatSnapshot => this.snapshot;

  state(): ChatSnapshot {
    return this.snapshot;
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  setDraft(text: string): void {
    this.desiredDraft = text;
    this.refresh();
    void this.flushDraft(this.epoch);
  }

  async send(): Promise<ChatSnapshot> {
    const epoch = this.epoch;
    await this.flushDraft(epoch);
    const text = this.desiredDraft.trim();
    if (!text || epoch !== this.epoch) return this.snapshot;
    await this.client.send(text);
    if (epoch === this.epoch) {
      this.desiredDraft = this.composer.state().text;
      this.refresh();
    }
    return this.snapshot;
  }

  newChat(): ChatSnapshot {
    this.bind(this.session.newChat());
    return this.snapshot;
  }

  selectChat(chatId: string): ChatSnapshot {
    this.bind(this.session.selectChat(chatId));
    return this.snapshot;
  }

  chatIds(): Array<string> {
    return this.session.chatIds();
  }

  history(): Array<ChatSummary> {
    return this.session.chatSummaries();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.epoch += 1;
    if (this.pollTimer) clearTimeout(this.pollTimer);
    this.chatStream.cancel();
    this.composerStream.cancel();
    this.composer.dispose();
    this.client.dispose();
    this.session.dispose();
    this.listeners.clear();
  }

  private bind(client: ffi.ChatHandle): void {
    this.epoch += 1;
    if (this.pollTimer) clearTimeout(this.pollTimer);
    if (this.client) {
      this.chatStream.cancel();
      this.composerStream.cancel();
      this.composer.dispose();
      this.client.dispose();
    }
    this.client = client;
    this.composer = client.composer();
    this.desiredDraft = this.composer.state().text;
    this.chatStream = client.chatChanges(this.refresh);
    this.composerStream = this.composer.composerChanges(this.composerInvalidated);
    this.refresh();
    const epoch = this.epoch;
    void this.composer.initialize().then((state) => {
      if (epoch !== this.epoch) return;
      if (!this.replacing) this.desiredDraft = state.text;
      this.refresh();
      this.schedulePoll(epoch);
    });
  }

  private flushDraft = (epoch: number): Promise<void> => {
    if (this.flushPromise) return this.flushPromise;
    this.replacing = true;
    this.flushPromise = (async () => {
      try {
        while (epoch === this.epoch && this.composer.state().text !== this.desiredDraft) {
          await this.composer.replace(this.desiredDraft);
        }
      } finally {
        this.replacing = false;
        this.flushPromise = undefined;
        if (epoch === this.epoch) {
          this.refresh();
        } else if (this.composer.state().text !== this.desiredDraft) {
          void this.flushDraft(this.epoch);
        }
      }
    })();
    return this.flushPromise;
  };

  private composerInvalidated = (): void => {
    if (!this.replacing) this.desiredDraft = this.composer.state().text;
    this.refresh();
  };

  private refresh = (): void => {
    if (this.disposed) return;
    const chat = this.client.state();
    const composer = this.composer.state();
    this.snapshot = {
      ...chat,
      chatId: this.client.id(),
      draft: this.desiredDraft,
      composerStatus: composer.status,
      composerError: composer.error,
    };
    [...this.listeners].forEach((listener) => listener());
  };

  private schedulePoll(epoch: number): void {
    this.pollTimer = setTimeout(async () => {
      if (this.disposed || epoch !== this.epoch) return;
      if (!this.replacing) {
        const state = await this.composer.syncOnce();
        if (epoch !== this.epoch) return;
        this.desiredDraft = state.text;
        this.refresh();
      }
      this.schedulePoll(epoch);
    }, 500);
  }
}
