export type AppSettings = {
  panel_width: number;
  hotkey: string;
  autostart: boolean;
  show_created_at: boolean;
  completed_retention_days: number;
  confirm_task_delete: boolean;
  purge_interval_hours: number;
  show_completed_tasks: boolean;
};

export type SettingsPatch = {
  hotkey?: string;
  autostart?: boolean;
  show_created_at?: boolean;
  completed_retention_days?: number;
  confirm_task_delete?: boolean;
  purge_interval_hours?: number;
  show_completed_tasks?: boolean;
};

export const DEFAULT_HOTKEY = "ctrl+shift+space";
