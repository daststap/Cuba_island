import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import { reportError, type ChatMessage } from "../../ipc/commands";
import { useSettingsStore } from "../settings/store";
import { chooseProvider, newChat, saveKey, sendMessage, stopReply, useChatStore } from "./store";

const enterInput = () => {
  useIslandStore.getState().enterInput();
};

/** Claude or Gemini: picks that provider's default model. */
function ProviderSwitch() {
  const provider = useChatStore((s) => s.provider);
  return (
    <div role="radiogroup" aria-label={ru.settings.chatModel} className="flex gap-1">
      {(["claude", "gemini"] as const).map((p) => (
        <button
          key={p}
          type="button"
          role="radio"
          aria-checked={provider === p}
          onClick={() => {
            chooseProvider(p);
          }}
          className={`rounded-full px-3 py-1 text-xs transition-colors ${
            provider === p ? "bg-white/20 text-white" : "text-white/60 hover:bg-white/10"
          }`}
        >
          {ru.chat.providers[p]} <span className="text-white/45">· {ru.chat.providerHint[p]}</span>
        </button>
      ))}
    </div>
  );
}

/** First run: choose the provider and paste its API key. */
export function KeySetup() {
  const provider = useChatStore((s) => s.provider);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = () => {
    if (!key.trim()) return;
    setBusy(true);
    saveKey(provider, key)
      .then(() => {
        setKey("");
      })
      .catch((error: unknown) => {
        reportError("chat_set_key", error);
      })
      .finally(() => {
        setBusy(false);
      });
  };
  return (
    <div className="flex w-full max-w-md flex-col gap-3 text-sm">
      <div className="flex items-center justify-between gap-3">
        <div className="text-base font-semibold">{ru.chat.setupTitle}</div>
        <ProviderSwitch />
      </div>
      <p className="text-[var(--island-muted)]">{ru.chat.setupText[provider]}</p>
      {provider === "gemini" && <p className="text-xs text-[#ff9f0a]">{ru.chat.geminiNote}</p>}
      <div className="flex gap-2">
        <input
          type="password"
          value={key}
          placeholder={ru.chat.keyPlaceholder[provider]}
          aria-label={ru.settings.apiKey(ru.chat.providers[provider] ?? provider)}
          autoComplete="off"
          spellCheck={false}
          onMouseDown={enterInput}
          onChange={(e) => {
            setKey(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
          }}
          className="min-w-0 flex-1 rounded-lg bg-white/10 px-3 py-2 outline-none placeholder:text-[var(--island-muted)] focus:bg-white/15"
        />
        <button
          type="button"
          disabled={busy || !key.trim()}
          onClick={submit}
          className="rounded-lg bg-[#0a84ff] px-3 py-2 font-medium text-white hover:bg-[#0a84ff]/85 disabled:opacity-40"
        >
          {ru.chat.saveKey}
        </button>
      </div>
    </div>
  );
}

function Bubble({ message }: { message: ChatMessage }) {
  const mine = message.role === "user";
  return (
    <li className={`flex ${mine ? "justify-end" : "justify-start"}`}>
      <div
        aria-label={mine ? ru.chat.you : ru.chat.assistant}
        className={`max-w-[85%] rounded-2xl px-3 py-2 text-sm leading-relaxed break-words whitespace-pre-wrap select-text ${
          mine ? "rounded-br-md bg-[#0a84ff] text-white" : "rounded-bl-md bg-white/10"
        }`}
      >
        {message.text}
      </div>
    </li>
  );
}

function Composer() {
  const streaming = useChatStore((s) => s.streaming);
  const [text, setText] = useState("");
  const send = () => {
    if (!text.trim()) return;
    sendMessage(text);
    setText("");
  };
  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!streaming) send();
    }
  };
  return (
    <div className="flex items-end gap-2">
      <textarea
        rows={1}
        value={text}
        placeholder={ru.chat.placeholder}
        aria-label={ru.chat.input}
        onMouseDown={enterInput}
        onChange={(e) => {
          setText(e.target.value);
        }}
        onKeyDown={onKeyDown}
        className="max-h-24 min-h-9 flex-1 resize-none rounded-xl bg-white/10 px-3 py-2 text-sm outline-none placeholder:text-[var(--island-muted)] focus:bg-white/15"
      />
      {streaming ? (
        <button
          type="button"
          onClick={stopReply}
          className="h-9 rounded-xl bg-white/15 px-3 text-xs font-medium hover:bg-white/25"
        >
          {ru.chat.stop}
        </button>
      ) : (
        <button
          type="button"
          disabled={!text.trim()}
          onClick={send}
          className="h-9 rounded-xl bg-[#0a84ff] px-3 text-xs font-medium text-white hover:bg-[#0a84ff]/85 disabled:opacity-40"
        >
          {ru.chat.send}
        </button>
      )}
    </div>
  );
}

/** The "Чат" tab: one conversation with Claude. */
export function ChatTab() {
  const { messages, streaming, hasKey, draft, notice, loaded } = useChatStore();
  const model = useSettingsStore((s) => s.settings?.chatModel ?? "");
  const end = useRef<HTMLLIElement>(null);

  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [messages.length, draft, notice]);

  if (!loaded) return null;
  if (!hasKey) return <KeySetup />;

  return (
    <div className="flex h-full min-h-0 w-full flex-col gap-2">
      <div className="flex items-center justify-between text-[11px] text-[var(--island-muted)]">
        <span>{ru.chat.models[model] ?? model}</span>
        <button
          type="button"
          disabled={streaming || messages.length === 0}
          onClick={newChat}
          className="rounded-full px-2 py-0.5 hover:bg-white/10 hover:text-[var(--island-fg)] disabled:opacity-40"
        >
          {ru.chat.newChat}
        </button>
      </div>
      {messages.length === 0 && !streaming ? (
        <div className="flex flex-1 items-center justify-center px-8 text-center text-sm text-[var(--island-muted)]">
          {ru.chat.empty}
        </div>
      ) : (
        <ul
          aria-label={ru.chat.label}
          className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto pr-1"
        >
          {messages.map((m, i) => (
            <Bubble key={i} message={m} />
          ))}
          {streaming &&
            (draft ? (
              <Bubble message={{ role: "assistant", text: draft }} />
            ) : (
              <li className="animate-pulse text-xs text-[var(--island-muted)]">
                {ru.chat.thinking}
              </li>
            ))}
          {notice && (
            <li role="alert" className="text-xs text-[#ff9f0a]">
              {notice}
            </li>
          )}
          <li ref={end} aria-hidden />
        </ul>
      )}
      <Composer />
    </div>
  );
}
