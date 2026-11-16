import { useEffect } from "react";
import { create } from "zustand";
import { ru } from "../../i18n/ru";
import {
  chatGet,
  chatNew,
  chatSend,
  chatSetKey,
  chatStop,
  reportError,
  type ChatMessage,
  type ChatState,
} from "../../ipc/commands";
import { onChatDelta, onChatDone } from "../../ipc/events";

interface ChatStore extends ChatState {
  /** The reply being streamed. */
  draft: string;
  /** A note under the conversation: an error, a refusal, a cut-off answer. */
  notice: string | null;
  loaded: boolean;
}

const NO_MESSAGES: ChatMessage[] = [];

export const useChatStore = create<ChatStore>()(() => ({
  messages: NO_MESSAGES,
  streaming: false,
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

/** Saves (or with "" removes) the API key; resolves once stored. */
export function saveKey(key: string): Promise<void> {
  return chatSetKey(key).then(apply);
}

/** Keeps the conversation in sync with Rust. Mount once. */
export function useChatSync(): void {
  useEffect(() => {
    chatGet().then(apply).catch(fail("chat_get"));
    const unsubscribe = [
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
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
