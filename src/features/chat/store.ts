import { useEffect } from "react";
import { create } from "zustand";
import { ru } from "../../i18n/ru";
import {
  CHAT_MODELS,
  chatGet,
  chatNew,
  chatSend,
  chatSetKey,
  chatStop,
  reportError,
  type ChatMessage,
  type ChatProvider,
  type ChatState,
} from "../../ipc/commands";
import { onChatDelta, onChatDone } from "../../ipc/events";
import { updateSettings, useSettingsStore } from "../settings/store";

interface ChatStore extends ChatState {
  /** The reply being streamed. */
  draft: string;
  /** A note under the conversation: an error, a refusal, a cut-off answer. */
  notice: string | null;
  loaded: boolean;
}

const NO_MESSAGES: ChatMessage[] = [];
/** Startup retries for the first load (Rust may still be setting up). */
const LOAD_ATTEMPTS = 10;
const LOAD_RETRY_MS = 300;

export const useChatStore = create<ChatStore>()(() => ({
  messages: NO_MESSAGES,
  streaming: false,
  provider: "claude",
  hasKey: false,
  draft: "",
  notice: null,
  loaded: false,
}));

function apply(state: ChatState) {
  useChatStore.setState({ ...state, loaded: true });
}

function fail(context: string) {
  return (error: unknown) => {
    reportError(context, error);
  };
}

/** Sends a question; the answer streams in. */
export function sendMessage(text: string): void {
  if (!text.trim() || useChatStore.getState().streaming) return;
  useChatStore.setState({ draft: "", notice: null });
  chatSend(text).then(apply).catch(fail("chat_send"));
}

export function stopReply(): void {
  chatStop().catch(fail("chat_stop"));
}

export function newChat(): void {
  useChatStore.setState({ draft: "", notice: null });
  chatNew().then(apply).catch(fail("chat_new"));
}

/** Saves (or with "" removes) a provider's API key; resolves once stored. */
export function saveKey(provider: ChatProvider, key: string): Promise<void> {
  return chatSetKey(provider, key).then(apply);
}

/** Switches the chat to a provider's default model. */
export function chooseProvider(provider: ChatProvider): void {
  const model = CHAT_MODELS[provider][0];
  if (model) void updateSettings({ chatModel: model });
}

/** Keeps the conversation in sync with Rust. Mount once. */
export function useChatSync(): void {
  useEffect(() => {
    let disposed = false;
    const reload = () => {
      chatGet().then(apply).catch(fail("chat_get"));
    };
    // At startup the page can ask before Rust has set the chat up: try again a few times.
    const load = (attempt: number) => {
      chatGet()
        .then(apply)
        .catch((error: unknown) => {
          if (disposed) return;
          if (attempt >= LOAD_ATTEMPTS) {
            reportError("chat_get", error);
            return;
          }
          setTimeout(() => {
            load(attempt + 1);
          }, LOAD_RETRY_MS);
        });
    };
    load(1);
    // Another model may mean another provider (and key).
    let model = useSettingsStore.getState().settings?.chatModel;
    const unsubscribeSettings = useSettingsStore.subscribe((s) => {
      const next = s.settings?.chatModel;
      if (next !== model) {
        model = next;
        reload();
      }
    });
    const unsubscribe = [
      unsubscribeSettings,
      onChatDelta((text) => {
        useChatStore.setState((s) => ({ draft: s.draft + text }));
      }),
      onChatDone((done) => {
        const notice =
          done.error ??
          (done.refused ? ru.chat.refused : done.truncated ? ru.chat.truncated : null);
        chatGet()
          .then((state) => {
            useChatStore.setState({ ...state, draft: "", notice, loaded: true });
          })
          .catch(fail("chat_get"));
      }),
    ];
    return () => {
      disposed = true;
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
