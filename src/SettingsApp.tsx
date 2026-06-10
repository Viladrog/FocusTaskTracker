import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { invokeWhenBackendReady } from "./lib/invokeWhenBackendReady";
import { type AppSettings, type SettingsPatch } from "./lib/appSettings";
import { buildShortcutFromEvent } from "./lib/buildShortcutFromEvent";
import { scheduleFitSettingsWindowWidth } from "./lib/fitSettingsWindow";
import { formatHotkeyLabel } from "./lib/formatHotkeyLabel";
import "./App.css";

async function patchSettings(patch: SettingsPatch): Promise<AppSettings> {
  return invoke<AppSettings>("settings_set", { patch });
}

function SettingsField({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="settings-field">
      <span className="settings-field-label">{label}</span>
      {children}
    </label>
  );
}

function SettingsApp() {
  const appRef = useRef<HTMLDivElement>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recordingHotkey, setRecordingHotkey] = useState(false);
  const [retentionDraft, setRetentionDraft] = useState("1");
  const [intervalDraft, setIntervalDraft] = useState("6");

  useEffect(() => {
    document.documentElement.classList.add("settings-window");
    document.body.classList.add("settings-window");
    return () => {
      document.documentElement.classList.remove("settings-window");
      document.body.classList.remove("settings-window");
    };
  }, []);

  useEffect(() => {
    void invokeWhenBackendReady<AppSettings>("settings_load")
      .then((loaded) => {
        setSettings(loaded);
        setRetentionDraft(String(loaded.completed_retention_days));
        setIntervalDraft(String(loaded.task_update_interval_hours));
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (!recordingHotkey) return;

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const shortcut = buildShortcutFromEvent(e);
      if (!shortcut) return;
      setRecordingHotkey(false);
      void patchSettings({ hotkey: shortcut })
        .then((updated) => {
          setSettings(updated);
          setError(null);
        })
        .catch((err) => setError(String(err)));
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recordingHotkey]);

  useEffect(() => {
    const el = appRef.current;
    if (!el) return;

    const fit = () => {
      scheduleFitSettingsWindowWidth(el);
    };

    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => observer.disconnect();
  }, [settings, error, recordingHotkey, retentionDraft, intervalDraft]);

  const applyBool = (patch: SettingsPatch) => {
    void patchSettings(patch)
      .then((updated) => {
        setSettings(updated);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  };

  const applyRetention = () => {
    const value = Number.parseInt(retentionDraft, 10);
    if (Number.isNaN(value)) {
      setError("Укажите целое число дней");
      return;
    }
    void patchSettings({ completed_retention_days: value })
      .then((updated) => {
        setSettings(updated);
        setRetentionDraft(String(updated.completed_retention_days));
        setError(null);
      })
      .catch((e) => setError(String(e)));
  };

  const applyInterval = () => {
    const value = Number.parseInt(intervalDraft, 10);
    if (Number.isNaN(value)) {
      setError("Укажите целое число часов");
      return;
    }
    void patchSettings({ task_update_interval_hours: value })
      .then((updated) => {
        setSettings(updated);
        setIntervalDraft(String(updated.task_update_interval_hours));
        setError(null);
      })
      .catch((e) => setError(String(e)));
  };

  if (!settings) {
    return (
      <div ref={appRef} className="settings-app">
        {error ? <p className="error">{error}</p> : <p className="settings-loading">Загрузка…</p>}
      </div>
    );
  }

  return (
    <div ref={appRef} className="settings-app">
      <h1 className="settings-title">Настройки</h1>

      {error ? <p className="error">{error}</p> : null}

      <section className="settings-section">
        <h2 className="settings-section-title">Задачи</h2>
        <SettingsField label="Использовать ежедневные">
          <input
            type="checkbox"
            checked={settings.use_daily}
            onChange={(e) => applyBool({ use_daily: e.target.checked })}
          />
        </SettingsField>
        <SettingsField label="Использовать еженедельные">
          <input
            type="checkbox"
            checked={settings.use_weekly}
            onChange={(e) => applyBool({ use_weekly: e.target.checked })}
          />
        </SettingsField>
        <SettingsField label="Использовать бэклог">
          <input
            type="checkbox"
            checked={settings.use_backlog}
            onChange={(e) => applyBool({ use_backlog: e.target.checked })}
          />
        </SettingsField>
        <SettingsField label="Отображать дату создания">
          <input
            type="checkbox"
            checked={settings.show_created_at}
            onChange={(e) => applyBool({ show_created_at: e.target.checked })}
          />
        </SettingsField>
        <SettingsField label="Показывать выполненные">
          <input
            type="checkbox"
            checked={settings.show_completed_tasks}
            onChange={(e) => applyBool({ show_completed_tasks: e.target.checked })}
          />
        </SettingsField>
        <SettingsField label="Подтверждать удаление">
          <input
            type="checkbox"
            checked={settings.confirm_task_delete}
            onChange={(e) => applyBool({ confirm_task_delete: e.target.checked })}
          />
        </SettingsField>
        <SettingsField label="Дней до удаления выполненных">
          <input
            type="number"
            className="settings-number"
            min={0}
            max={365}
            value={retentionDraft}
            onChange={(e) => setRetentionDraft(e.target.value)}
            onBlur={() => applyRetention()}
            onKeyDown={(e) => {
              if (e.key === "Enter") applyRetention();
            }}
          />
        </SettingsField>
        <SettingsField label="Интервал обновления задач (ч)">
          <input
            type="number"
            className="settings-number"
            min={1}
            max={168}
            value={intervalDraft}
            onChange={(e) => setIntervalDraft(e.target.value)}
            onBlur={() => applyInterval()}
            onKeyDown={(e) => {
              if (e.key === "Enter") applyInterval();
            }}
          />
        </SettingsField>
      </section>

      <section className="settings-section">
        <h2 className="settings-section-title">Окно</h2>
        <SettingsField label="Глобальный хоткей">
          <div className="settings-field-control">
            <span className="settings-hotkey-value">
              {recordingHotkey ? "Нажмите сочетание…" : formatHotkeyLabel(settings.hotkey)}
            </span>
            <button
              type="button"
              className="settings-action-btn"
              onClick={() => setRecordingHotkey(true)}
            >
              Изменить
            </button>
          </div>
        </SettingsField>
      </section>

      <section className="settings-section">
        <h2 className="settings-section-title">Система</h2>
        <SettingsField label="Автозапуск">
          <input
            type="checkbox"
            checked={settings.autostart}
            onChange={(e) => applyBool({ autostart: e.target.checked })}
          />
        </SettingsField>
      </section>
    </div>
  );
}

export default SettingsApp;
