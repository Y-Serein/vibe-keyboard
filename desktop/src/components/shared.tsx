import React from "react";

// ── Pressable Button ──

interface PressableButtonProps {
  onClick?: () => void;
  onMouseDown?: () => void;
  onMouseUp?: () => void;
  style: React.CSSProperties;
  pressedStyle?: React.CSSProperties;
  children: React.ReactNode;
  disabled?: boolean;
  'aria-label'?: string;
}

export function PressableButton({
  onClick, onMouseDown, onMouseUp, style, pressedStyle, children, disabled, ...rest
}: PressableButtonProps) {
  const [isPressed, setIsPressed] = React.useState(false);
  const merged = isPressed && pressedStyle ? { ...style, ...pressedStyle } : style;
  return (
    <button
      style={merged}
      disabled={disabled}
      onClick={onClick}
      onMouseDown={() => { setIsPressed(true); onMouseDown?.(); }}
      onMouseUp={() => { setIsPressed(false); onMouseUp?.(); }}
      onMouseLeave={() => { setIsPressed(false); }}
      {...rest}
    >
      {children}
    </button>
  );
}

// ── Section Card ──

interface SectionCardProps {
  title?: string;
  children: React.ReactNode;
  style?: React.CSSProperties;
}

export function SectionCard({ title, children, style }: SectionCardProps) {
  return (
    <div
      style={{
        background: "var(--surface)",
        borderRadius: "var(--radius-lg)",
        padding: "16px 18px",
        marginBottom: "var(--space-md)",
        boxShadow: "var(--shadow-card)",
        ...style,
      }}
    >
      {title && (
        <h3
          style={{
            fontSize: "var(--font-size-lg)",
            color: "var(--text)",
            marginBottom: "12px",
            fontWeight: 700,
          }}
        >
          {title}
        </h3>
      )}
      {children}
    </div>
  );
}

// ── Toggle Switch ──

interface ToggleProps {
  active: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

export function Toggle({ active, onChange, disabled }: ToggleProps) {
  return (
    <button
      role="switch"
      aria-checked={active}
      disabled={disabled}
      onClick={() => onChange(!active)}
      style={{
        width: "40px",
        height: "22px",
        borderRadius: "11px",
        border: "none",
        cursor: disabled ? "not-allowed" : "pointer",
        background: disabled
          ? "#e5e7eb"
          : active
            ? "var(--success)"
            : "var(--border)",
        position: "relative",
        transition: "background 0.2s",
        flexShrink: 0,
        opacity: disabled ? 0.5 : 1,
      }}
    >
      <div
        style={{
          position: "absolute",
          top: "3px",
          left: active ? "20px" : "3px",
          width: "16px",
          height: "16px",
          borderRadius: "50%",
          background: "#fff",
          boxShadow: "0 1px 3px rgba(0,0,0,0.2)",
          transition: "left 0.2s",
        }}
      />
    </button>
  );
}

// ── Status Badge ──

type BadgeVariant = "success" | "danger" | "warning" | "info" | "muted";

interface StatusBadgeProps {
  variant: BadgeVariant;
  label: string;
}

const badgeColors: Record<BadgeVariant, string> = {
  success: "var(--success)",
  danger: "var(--danger)",
  warning: "var(--warning)",
  info: "var(--info)",
  muted: "var(--text-muted)",
};

const badgeIcons: Record<BadgeVariant, string> = {
  success: "\u2713",
  danger: "\u2717",
  warning: "\u25CF",
  info: "\u25CF",
  muted: "\u25CB",
};

export function StatusBadge({ variant, label }: StatusBadgeProps) {
  const color = badgeColors[variant];
  return (
    <span
      style={{
        fontSize: "var(--font-size-sm)",
        fontWeight: 700,
        color,
      }}
    >
      {badgeIcons[variant]} {label}
    </span>
  );
}

// ── Action Button ──

type ButtonVariant = "primary" | "secondary" | "danger" | "success" | "ghost";

interface ActionButtonProps {
  variant?: ButtonVariant;
  children: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  style?: React.CSSProperties;
  fullWidth?: boolean;
}

export function ActionButton({
  variant = "primary",
  children,
  onClick,
  disabled,
  style,
  fullWidth,
}: ActionButtonProps) {
  const base: React.CSSProperties = {
    padding: "8px 16px",
    fontSize: "var(--font-size-md)",
    fontWeight: 600,
    border: "none",
    borderRadius: "var(--radius-sm)",
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.5 : 1,
    transition: "all 0.15s",
    width: fullWidth ? "100%" : undefined,
  };

  const variants: Record<ButtonVariant, React.CSSProperties> = {
    primary: { background: "var(--primary)", color: "#fff" },
    secondary: { background: "var(--primary-light)", color: "var(--text)" },
    danger: {
      background: "transparent",
      color: "var(--danger)",
      border: "1px solid var(--danger-light)",
    },
    success: { background: "var(--success)", color: "#fff" },
    ghost: {
      background: "transparent",
      color: "var(--text-secondary)",
      border: "1px solid var(--border)",
    },
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{ ...base, ...variants[variant], ...style }}
    >
      {children}
    </button>
  );
}

// ── Tab Bar ──

interface TabBarProps<T extends string> {
  tabs: { id: T; label: string }[];
  active: T;
  onChange: (tab: T) => void;
}

export function TabBar<T extends string>({
  tabs,
  active,
  onChange,
}: TabBarProps<T>) {
  return (
    <div role="tablist" style={{ display: "flex", gap: "var(--space-sm)", marginBottom: "20px" }}>
      {tabs.map((tab) => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={tab.id === active}
          onClick={() => onChange(tab.id)}
          style={{
            padding: "6px 16px",
            fontSize: "var(--font-size-md)",
            fontWeight: 600,
            background:
              tab.id === active ? "var(--primary)" : "var(--primary-light)",
            color: tab.id === active ? "#fff" : "#666",
            border: "none",
            borderRadius: "8px",
            cursor: "pointer",
            transition: "all 0.15s",
          }}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

// ── Spinner ──

export function Spinner({ size = 16 }: { size?: number }) {
  return (
    <div
      style={{
        width: size,
        height: size,
        border: `2px solid var(--border)`,
        borderTopColor: "var(--primary)",
        borderRadius: "50%",
        animation: "spin 0.6s linear infinite",
        display: "inline-block",
      }}
    />
  );
}

// ── Label ──

export function Label({ children }: { children: React.ReactNode }) {
  return (
    <span
      style={{
        fontSize: "var(--font-size-sm)",
        fontWeight: 600,
        color: "var(--text-secondary)",
      }}
    >
      {children}
    </span>
  );
}

// ── Row ──

interface RowProps {
  label: string;
  children: React.ReactNode;
  description?: string;
}

export function SettingRow({ label, children, description }: RowProps) {
  return (
    <div style={{ marginBottom: "12px" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: description ? "4px" : 0,
        }}
      >
        <Label>{label}</Label>
        {children}
      </div>
      {description && (
        <p
          style={{
            fontSize: "var(--font-size-xs)",
            color: "var(--text-muted)",
            marginTop: "2px",
          }}
        >
          {description}
        </p>
      )}
    </div>
  );
}
