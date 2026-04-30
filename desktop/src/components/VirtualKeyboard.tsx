import { PressableButton } from "./shared";

interface VirtualKeyboardProps {
  screenMode: "standby" | "normal" | "select" | "allow" | "notify";
  modeYolo: boolean;
  hasAlert: boolean;
  notificationCount?: number;
  onButtonPress: (buttonId: string, action?: string) => void;
  onButtonDown?: (buttonId: string) => void;
  onButtonUp?: (buttonId: string) => void;
  onKnobAction: (action: string) => void;
}

const btnBase: React.CSSProperties = {
  borderRadius: "10px",
  border: "none",
  cursor: "pointer",
  fontFamily: "'Helvetica Neue', Arial, sans-serif",
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: "0.3px",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: "2px",
  position: "relative",
  transition: "all 0.1s",
  background: "linear-gradient(180deg, #f7f5f9 0%, #dedad4 100%)",
  boxShadow:
    "0 2px 6px rgba(0,0,0,0.14), inset 0 1px 0 rgba(255,255,255,0.9)",
  color: "#444",
};

const btnSm: React.CSSProperties = {
  ...btnBase,
  padding: "8px 4px",
  fontSize: "10px",
  minHeight: "50px",
};

const btnLg: React.CSSProperties = {
  ...btnBase,
  padding: "14px 8px",
  fontSize: "14px",
  minHeight: "52px",
};

const redBorder: React.CSSProperties = {
  color: "#dc2626",
  border: "1.5px solid #fca5a5",
};

const pressedStyle: React.CSSProperties = {
  transform: "translateY(1px)",
  boxShadow: "0 1px 2px rgba(0,0,0,0.15)",
};

const pressedKnobStyle: React.CSSProperties = {
  transform: "translateY(1px)",
};

const ledStyle = (
  color: string,
  blink: boolean = false,
): React.CSSProperties => ({
  width: "6px",
  height: "6px",
  borderRadius: "50%",
  position: "absolute",
  top: "6px",
  right: "6px",
  background: color,
  boxShadow: `0 0 6px ${color}`,
  animation: blink ? "blink 1s infinite" : "none",
});

const knobBtnStyle: React.CSSProperties = {
  ...btnBase,
  padding: "4px 8px",
  fontSize: "14px",
  minHeight: "28px",
  width: "56px",
};

function VirtualKeyboard({
  screenMode,
  modeYolo,
  hasAlert,
  onButtonPress,
  onButtonDown,
  onButtonUp,
  onKnobAction,
  notificationCount = 0,
}: VirtualKeyboardProps) {
  const isAllow = screenMode === "allow";

  const sendStyle: React.CSSProperties = isAllow
    ? {
        ...btnLg,
        background: "linear-gradient(180deg, #d1fae5 0%, #6ee7b7 100%)",
        color: "#065f46",
      }
    : { ...btnLg };

  const knobLabelColor = isAllow ? "#15803d" : "#888";
  const knobLabel = isAllow ? "ALLOW SELECT" : "SESSION BROWSE";

  const _handleButton = (id: string) => {
    onButtonPress(id);
  };

  const _handleKnob = (action: string) => {
    onKnobAction(action);
  };

  return (
    <div>
      {/* Middle: button grid + knob */}
      <div style={{ display: "flex", gap: "12px", marginBottom: "14px" }}>
        {/* 2x2 button grid */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: "8px",
            flex: 1,
          }}
        >
          {/* DELETE -- real-time: hold=key down, release=key up */}
          <PressableButton
            style={{ ...btnSm, ...redBorder }}
            pressedStyle={pressedStyle}
            onMouseDown={() => {
              if (!document.hasFocus()) return;
              onButtonDown?.("delete");
            }}
            onMouseUp={() => onButtonUp?.("delete")}
            aria-label="Delete - erase input"
          >
            <span style={{ fontSize: "15px" }}>{"\u2297"}</span>
            DELETE
            <span style={{ fontSize: "7px", color: "#999", fontWeight: 400, marginTop: "1px" }}>
              ERASE INPUT
            </span>
          </PressableButton>

          {/* CANCEL */}
          <PressableButton
            style={{ ...btnSm, ...redBorder }}
            pressedStyle={pressedStyle}
            onClick={() => _handleButton("cancel")}
            aria-label="Cancel - stop AI"
          >
            <span style={{ fontSize: "15px" }}>{"\u2718"}</span>
            CANCEL
            <span style={{ fontSize: "7px", color: "#999", fontWeight: 400, marginTop: "1px" }}>
              {isAllow ? "QUICK DENY" : "STOP AI"}
            </span>
          </PressableButton>

          {/* MODE */}
          <PressableButton
            style={{ ...btnSm, position: "relative" }}
            pressedStyle={pressedStyle}
            onClick={() => _handleButton("mode")}
            aria-label="Mode - toggle Plan/YOLO"
          >
            <span style={ledStyle(modeYolo ? "#f59e0b" : "#22c55e", false)} />
            <span style={{ fontSize: "15px" }}>{"\u26A1"}</span>
            MODE
            <span style={{ fontSize: "7px", color: "#999", fontWeight: 400, marginTop: "1px" }}>
              PLAN / YOLO
            </span>
          </PressableButton>

          {/* SESSION */}
          <PressableButton
            style={{ ...btnSm, position: "relative" }}
            pressedStyle={pressedStyle}
            onClick={() => _handleButton("session")}
            aria-label="Notify - notifications"
          >
            {(hasAlert || notificationCount > 0) && (
              <span style={ledStyle(hasAlert ? "#ef4444" : "#3b82f6", hasAlert)} />
            )}
            <span style={{ fontSize: "15px" }}>{"\uD83D\uDD14"}</span>
            NOTIFY
            <span style={{ fontSize: "7px", color: "#999", fontWeight: 400, marginTop: "1px" }}>
              {notificationCount > 0
                ? `${notificationCount} UNREAD`
                : "NOTIFICATIONS"}
            </span>
          </PressableButton>
        </div>

        {/* Knob column -- THREE buttons */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            width: "76px",
            flexShrink: 0,
            gap: "6px",
          }}
        >
          {/* CCW (up arrow) */}
          <PressableButton
            style={{
              ...knobBtnStyle,
              background: isAllow
                ? "linear-gradient(180deg, #d1fae5 0%, #a7f3d0 100%)"
                : knobBtnStyle.background,
              color: isAllow ? "#065f46" : "#444",
            }}
            pressedStyle={pressedKnobStyle}
            onClick={() => _handleKnob("ccw")}
            aria-label="Previous session"
          >
            {"\u25B2"}
          </PressableButton>

          {/* PRESS (center dot) */}
          <PressableButton
            style={{
              ...knobBtnStyle,
              background: isAllow
                ? "linear-gradient(180deg, #bbf7d0 0%, #6ee7b7 100%)"
                : "linear-gradient(180deg, #fde68a 0%, #f59e0b 100%)",
              color: isAllow ? "#065f46" : "#78350f",
              fontWeight: 800,
            }}
            pressedStyle={pressedKnobStyle}
            onClick={() => _handleKnob("press")}
            aria-label="Confirm selection"
          >
            {"\u25CF"}
          </PressableButton>

          {/* CW (down arrow) */}
          <PressableButton
            style={{
              ...knobBtnStyle,
              background: isAllow
                ? "linear-gradient(180deg, #d1fae5 0%, #a7f3d0 100%)"
                : knobBtnStyle.background,
              color: isAllow ? "#065f46" : "#444",
            }}
            pressedStyle={pressedKnobStyle}
            onClick={() => _handleKnob("cw")}
            aria-label="Next session"
          >
            {"\u25BC"}
          </PressableButton>

          <div
            style={{
              textAlign: "center",
              fontSize: "8px",
              color: knobLabelColor,
              fontWeight: 700,
              textTransform: "uppercase",
              letterSpacing: "0.5px",
            }}
          >
            {knobLabel}
          </div>
        </div>
      </div>

      {/* Bottom row: SEND + VOICE */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: "10px",
        }}
      >
        <PressableButton
          style={sendStyle}
          pressedStyle={pressedStyle}
          onClick={() => _handleButton("send")}
          aria-label={isAllow ? "Send - allow" : "Send"}
        >
          <span style={{ fontSize: "17px", marginBottom: "1px" }}>
            {isAllow ? "\u2713" : "\u21B5"}
          </span>
          SEND
          {isAllow && (
            <span style={{ fontSize: "8px", color: "#065f46", fontWeight: 400 }}>
              (= ALLOW)
            </span>
          )}
        </PressableButton>

        {/* VOICE -- click mode */}
        <PressableButton
          style={{ ...btnLg }}
          pressedStyle={pressedStyle}
          onClick={() => _handleButton("voice")}
          aria-label="Voice"
        >
          <span style={{ fontSize: "17px", marginBottom: "1px" }}>
            {"\uD83C\uDFA4"}
          </span>
          VOICE
        </PressableButton>
      </div>
    </div>
  );
}

export default VirtualKeyboard;
