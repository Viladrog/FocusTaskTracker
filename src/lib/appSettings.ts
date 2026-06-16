export type AppSettings = {
  panel_width: number;
  hotkey: string;
  autostart: boolean;
  show_created_at: boolean;
  show_completed_at: boolean;
  completed_retention_days: number;
  confirm_task_delete: boolean;
  task_update_interval_hours: number;
  show_completed_tasks: boolean;
  use_daily: boolean;
  use_weekly: boolean;
  use_backlog: boolean;
};

export type SettingsPatch = {
  hotkey?: string;
  autostart?: boolean;
  show_created_at?: boolean;
  show_completed_at?: boolean;
  completed_retention_days?: number;
  confirm_task_delete?: boolean;
  task_update_interval_hours?: number;
  show_completed_tasks?: boolean;
  use_daily?: boolean;
  use_weekly?: boolean;
  use_backlog?: boolean;
};

export const DEFAULT_HOTKEY = "ctrl+shift+space";
