export interface DaemonConfig {
  macros?: Record<string, string>;
  sound?: {
    enabled?: boolean;
    volume?: number;
    muted?: boolean;
    mapping?: Record<string, string>;
  };
  yolo?: {
    active?: boolean;
    allow?: string[];
    deny?: string[];
    notify_auto_allow?: boolean;
    auto_allow_log?: boolean;
  };
  general?: {
    hook_port?: number;
    log_level?: string;
  };
  ipc?: {
    socket_path?: string;
  };
}
