import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState, type KeyboardEvent, type ReactNode } from "react";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import {
  CHAT_MODELS,
  type ScreenEdge,
  notificationsOpenSettings,
  clipboardClear,
  hotkeysPause,
  monitorsList,
  reportError,
  updateCheck,
  updateInstall,
  type MonitorInfo,
  type Settings,
} from "../../ipc/commands";
import { ClaudeHooksSetting } from "../claude/views";
import { useClipboardStore } from "../clipboard/store";
import { comboFromKeyPress } from "./hotkey";
import { KeySetup } from "../chat/ChatTab";
import { useNotificationsStore } from "../notifications/store";
import { saveKey, useChatStore } from "../chat/store";
import { updateSettings, useSettingsStore } from "./store";

const HISTORY_LIMITS = [100, 250, 500, 1000, 2000];

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-9 items-center justify-between gap-4 py-1">
      <span className="text-sm">{label}</span>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <Row label={label}>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        onClick={() => {
          onChange(!checked);
        }}
        className={`relative h-6 w-10 rounded-full transition-colors ${
          checked ? "bg-[#30d158]" : "bg-white/20"
        }`}
      >
        <span
          className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-[left] ${
            checked ? "left-[18px]" : "left-0.5"
          }`}
        />
      </button>
    </Row>
  );
}

const control =
  "rounded-lg bg-white/10 px-2 py-1 text-xs text-[var(--island-fg)] outline-none hover:bg-white/15";

/** Click, then press a combination; Esc cancels. Our hotkeys are paused meanwhile. */
function HotkeyField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (combo: string) => void;
}) {
  const [recording, setRecording] = useState(false);

  const stop = () => {
    setRecording(false);
    hotkeysPause(false).catch((error: unknown) => {
      reportError("hotkeys_pause", error);
    });
  };

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      stop();
      return;
    }
    const combo = comboFromKeyPress(e);
    if (!combo) return;
    stop();
    if (combo !== value) onChange(combo);
  };

  return (
    <Row label={label}>
      <button
        type="button"
        aria-label={`${label}: ${value}`}
        title={ru.settings.recordHint}
        onClick={() => {
          useIslandStore.getState().enterInput();
          setRecording(true);
          hotkeysPause(true).catch((error: unknown) => {
            reportError("hotkeys_pause", error);
          });
        }}
        onBlur={() => {
          if (recording) stop();
        }}
        onKeyDown={onKeyDown}
        className={`${control} min-w-28 font-mono ${recording ? "ring-1 ring-[#0a84ff]" : ""}`}
      >
        {recording ? ru.settings.recording : value}
      </button>
    </Row>
  );
}

function ClearHistory() {
  const [confirming, setConfirming] = useState(false);
  return (
    <button
      type="button"
      className={`${control} ${confirming ? "bg-[#ff453a]/30 hover:bg-[#ff453a]/40" : ""}`}
      onBlur={() => {
        setConfirming(false);
      }}
      onClick={() => {
        if (!confirming) {
          setConfirming(true);
          return;
        }
        setConfirming(false);
        clipboardClear()
          .then(() => {
            useClipboardStore.getState().bump();
            useIslandStore.getState().showToast(ru.settings.cleared);
          })
          .catch((error: unknown) => {
            reportError("clipboard_clear", error);
          });
      }}
    >
      {confirming ? ru.settings.clearConfirm : ru.settings.clearHistory}
    </button>
  );
}

function Updates() {
  const update = useSettingsStore((s) => s.update);
  const [version, setVersion] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {
        setVersion("");
      });
  }, []);

  return (
    <Row label={ru.settings.version(version)}>
      {status && <span className="text-xs text-[var(--island-muted)]">{status}</span>}
      {update ? (
        <button
          type="button"
          className={`${control} bg-[#0a84ff]/40 hover:bg-[#0a84ff]/60`}
          onClick={() => {
            setStatus(ru.settings.installing);
            updateInstall().catch((error: unknown) => {
              setStatus(null);
              reportError("update_install", error);
            });
          }}
        >
          {ru.settings.installUpdate(update.version)}
        </button>
      ) : (
        <button
          type="button"
          className={control}
          onClick={() => {
            updateCheck()
              .then((found) => {
                useSettingsStore.setState({ update: found });
                if (!found) setStatus(ru.settings.upToDate);
              })
              .catch((error: unknown) => {
                reportError("update_check", error);
              });
          }}
        >
          {ru.settings.checkUpdates}
        </button>
      )}
    </Row>
  );
}

function SettingsForm({ settings }: { settings: Settings }) {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  useEffect(() => {
    monitorsList()
      .then(setMonitors)
      .catch((error: unknown) => {
        reportError("monitors_list", error);
      });
  }, []);
  const set = (patch: Partial<Settings>) => {
    void updateSettings(patch);
  };

  return (
    <div className="divide-y divide-white/8">
      {monitors.length > 1 && (
        <Row label={ru.settings.monitor}>
          <select
            aria-label={ru.settings.monitor}
            className={control}
            value={settings.monitor ?? ""}
            onChange={(e) => {
              set({ monitor: e.target.value || null });
            }}
          >
            <option value="">{ru.settings.primaryMonitor}</option>
            {monitors.map((m, i) => (
              <option key={m.name} value={m.name}>
                {ru.settings.monitorLabel(i + 1, m.width, m.height, m.primary)}
              </option>
            ))}
          </select>
        </Row>
      )}
      <Row label={ru.settings.edge}>
        <select
          aria-label={ru.settings.edge}
          title={ru.settings.edgeHint}
          className={control}
          value={settings.edge}
          onChange={(e) => {
            set({ edge: e.target.value as ScreenEdge });
          }}
        >
          {(["top", "bottom", "left", "right"] as const).map((edge) => (
            <option key={edge} value={edge}>
              {ru.settings.edges[edge]}
            </option>
          ))}
        </select>
        <button
          type="button"
          disabled={settings.edgeOffset === 0.5}
          className="rounded-lg bg-white/10 px-2 py-1 text-xs hover:bg-white/20 disabled:opacity-40"
          onClick={() => {
            set({ edgeOffset: 0.5 });
          }}
        >
          {ru.settings.edgeCentre}
        </button>
      </Row>
      <Toggle
        label={ru.settings.autostart}
        checked={settings.autostart}
        onChange={(autostart) => {
          set({ autostart });
        }}
      />
      <Toggle
        label={ru.settings.hideInFullscreen}
        checked={settings.hideInFullscreen}
        onChange={(hideInFullscreen) => {
          set({ hideInFullscreen });
        }}
      />
      <Row label={ru.settings.timeFormat}>
        {([false, true] as const).map((hour12) => (
          <button
            key={String(hour12)}
            type="button"
            aria-pressed={settings.hour12 === hour12}
            className={`${control} ${settings.hour12 === hour12 ? "bg-white/25" : ""}`}
            onClick={() => {
              set({ hour12 });
            }}
          >
            {hour12 ? ru.settings.time12 : ru.settings.time24}
          </button>
        ))}
      </Row>
      <HotkeyField
        label={ru.settings.clipboardHotkey}
        value={settings.clipboardHotkey}
        onChange={(clipboardHotkey) => {
          set({ clipboardHotkey });
        }}
      />
      <HotkeyField
        label={ru.settings.launcherHotkey}
        value={settings.launcherHotkey}
        onChange={(launcherHotkey) => {
          set({ launcherHotkey });
        }}
      />
      <HotkeyField
        label={ru.settings.screenshotHotkey}
        value={settings.screenshotHotkey}
        onChange={(screenshotHotkey) => {
          set({ screenshotHotkey });
        }}
      />
      <Row label={ru.settings.chatModel}>
        <select
          aria-label={ru.settings.chatModel}
          className={control}
          value={settings.chatModel}
          onChange={(e) => {
            set({ chatModel: e.target.value });
          }}
        >
          {(["claude", "gemini"] as const).map((provider) => (
            <optgroup key={provider} label={ru.chat.providers[provider]}>
              {CHAT_MODELS[provider].map((model) => (
                <option key={model} value={model}>
                  {ru.chat.models[model] ?? model}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </Row>
      <ApiKeyRow />
      <Row label={ru.settings.historyLimit}>
        <select
          aria-label={ru.settings.historyLimit}
          className={control}
          value={settings.historyLimit}
          onChange={(e) => {
            set({ historyLimit: Number(e.target.value) });
          }}
        >
          {[...new Set([...HISTORY_LIMITS, settings.historyLimit])]
            .sort((a, b) => a - b)
            .map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
        </select>
        <ClearHistory />
      </Row>
      <Toggle
        label={ru.settings.copyPeek}
        checked={settings.copyPeek}
        onChange={(copyPeek) => {
          set({ copyPeek });
        }}
      />
      <Toggle
        label={ru.settings.privacyDots}
        checked={settings.privacyDots}
        onChange={(privacyDots) => {
          set({ privacyDots });
        }}
      />
      <Toggle
        label={ru.settings.capsPeek}
        checked={settings.capsPeek}
        onChange={(capsPeek) => {
          set({ capsPeek });
        }}
      />
      <NotificationSettings
        enabled={settings.notifications}
        muted={settings.notificationsMuted}
        set={set}
      />
      <Toggle
        label={ru.settings.downloads}
        checked={settings.downloads}
        onChange={(downloads) => {
          set({ downloads });
        }}
      />
      <Row label={ru.settings.opacity}>
        <input
          type="range"
          aria-label={ru.settings.opacity}
          min={50}
          max={100}
          step={5}
          value={Math.round(settings.opacity * 100)}
          onChange={(e) => {
            set({ opacity: Number(e.target.value) / 100 });
          }}
          className="w-32 accent-[#0a84ff]"
        />
        <span className="w-9 text-right text-xs tabular-nums text-[var(--island-muted)]">
          {Math.round(settings.opacity * 100)}%
        </span>
      </Row>
      <Row label={ru.claude.setting}>
        <ClaudeHooksSetting className={control} />
      </Row>
      <Updates />
    </div>
  );
}

/** The "Настройки" tab (PLAN.md stage 5). */
/** The API key for the chat: saved or not; the key itself never comes back. */
function ApiKeyRow() {
  const hasKey = useChatStore((st) => st.hasKey);
  const provider = useChatStore((st) => st.provider);
  if (!hasKey) {
    return (
      <div className="border-b border-white/8 py-2">
        <KeySetup />
      </div>
    );
  }
  return (
    <Row label={ru.settings.apiKey(ru.chat.providers[provider] ?? provider)}>
      <span className="text-xs text-[var(--island-muted)]">{ru.settings.keySaved}</span>
      <button
        type="button"
        className="rounded-lg bg-white/10 px-2 py-1 text-xs hover:bg-white/20"
        onClick={() => {
          saveKey(provider, "").catch((error: unknown) => {
            reportError("chat_set_key", error);
          });
        }}
      >
        {ru.settings.removeKey}
      </button>
    </Row>
  );
}

/** Windows notifications: on/off, access hint, and a switch per app. */
function NotificationSettings({
  enabled,
  muted,
  set,
}: {
  enabled: boolean;
  muted: string[];
  set: (patch: { notifications?: boolean; notificationsMuted?: string[] }) => void;
}) {
  const access = useNotificationsStore((s) => s.access);
  const apps = useNotificationsStore((s) => s.apps);
  return (
    <>
      <Toggle
        label={ru.settings.notifications}
        checked={enabled}
        onChange={(notifications) => {
          set({ notifications });
        }}
      />
      {enabled && access !== "allowed" && (
        <div className="flex items-center justify-between gap-3 py-1 text-xs text-[#ff9f0a]">
          <span>{ru.settings.notificationsDenied}</span>
          <button
            type="button"
            className="shrink-0 rounded-lg bg-white/10 px-2 py-1 text-[var(--island-fg)] hover:bg-white/20"
            onClick={() => {
              notificationsOpenSettings().catch((error: unknown) => {
                reportError("notifications_open_settings", error);
              });
            }}
          >
            {ru.settings.openPrivacy}
          </button>
        </div>
      )}
      {enabled && access === "allowed" && (
        <div className="py-1">
          <div className="text-xs text-[var(--island-muted)]">{ru.settings.notificationApps}</div>
          {apps.length === 0 ? (
            <div className="py-1 text-xs text-[var(--island-muted)]">
              {ru.settings.notificationAppsEmpty}
            </div>
          ) : (
            <div className="pl-3">
              {apps.map((app) => {
                const on = !muted.some((m) => m.toLowerCase() === app.toLowerCase());
                return (
                  <Toggle
                    key={app}
                    label={app}
                    checked={on}
                    onChange={(show) => {
                      set({
                        notificationsMuted: show
                          ? muted.filter((m) => m.toLowerCase() !== app.toLowerCase())
                          : [...muted, app],
                      });
                    }}
                  />
                );
              })}
            </div>
          )}
        </div>
      )}
    </>
  );
}

export function SettingsTab() {
  const settings = useSettingsStore((s) => s.settings);
  return (
    <div className="min-h-0 w-full flex-1 self-stretch overflow-y-auto pr-1">
      {settings && <SettingsForm settings={settings} />}
    </div>
  );
}

/** First-run question: start with Windows? Shown over the panel until answered. */
export function Onboarding() {
  const answer = (autostart: boolean) => {
    void updateSettings({ autostart, onboarded: true }).then((ok) => {
      if (ok) useIslandStore.getState().dispatch({ type: "collapse" });
    });
  };
  return (
    <div
      role="dialog"
      aria-label={ru.onboarding.question}
      className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-[var(--island-bg)] px-10 text-center"
    >
      <div className="text-lg font-semibold">{ru.onboarding.question}</div>
      <div className="text-sm text-[var(--island-muted)]">{ru.onboarding.hint}</div>
      <div className="mt-2 flex gap-3">
        <button
          type="button"
          className="rounded-xl bg-[#0a84ff] px-6 py-2 text-sm font-semibold"
          onClick={() => {
            answer(true);
          }}
        >
          {ru.onboarding.yes}
        </button>
        <button
          type="button"
          className="rounded-xl bg-white/15 px-6 py-2 text-sm"
          onClick={() => {
            answer(false);
          }}
        >
          {ru.onboarding.no}
        </button>
      </div>
    </div>
  );
}
