//! Binary codec for protocol messages.
//!
//! Wire format: `[1B tag][fields...]`
//! - Strings: `[2B LE length][utf8 bytes]`
//! - `Vec<SessionInfo>`: `[1B count][session1...]`
//! - `LedColor`: `[3B r, g, b]`

use crate::message::{
    ButtonId, Direction, DownlinkMessage, LedColor, NotificationInfo, PermissionAction,
    SessionInfo, SessionStatus, SoundType, UplinkMessage,
};
use std::fmt;

// ── Tag constants ──

const TAG_BUTTON_PRESS: u8 = 0x01;
const TAG_BUTTON_RELEASE: u8 = 0x02;
const TAG_KNOB_ROTATE: u8 = 0x03;
const TAG_KNOB_PRESS: u8 = 0x04;
const TAG_KNOB_RELEASE: u8 = 0x05;
const TAG_PERMISSION_RESPONSE: u8 = 0x06;
const TAG_SESSION_SWITCH: u8 = 0x07;

const TAG_SESSION_LIST_UPDATE: u8 = 0x81;
const TAG_SESSION_STATUS_CHANGE: u8 = 0x82;
const TAG_PERMISSION_REQUEST: u8 = 0x83;
const TAG_SET_LED: u8 = 0x84;
const TAG_SET_KNOB_RING: u8 = 0x85;
const TAG_PLAY_SOUND: u8 = 0x86;
const TAG_DISMISS_PERMISSION: u8 = 0x87;
const TAG_FRAME_DATA: u8 = 0x88;
const TAG_NOTIFICATION_LIST_UPDATE: u8 = 0x89;
const TAG_SET_VOLUME: u8 = 0x8A;
const TAG_SET_MUTED: u8 = 0x8B;
const TAG_SET_SOUND_MAPPING: u8 = 0x8C;

// ── Error ──

/// Codec error type.
#[derive(Debug, PartialEq, Eq)]
pub enum CodecError {
    /// Unknown or invalid tag byte.
    InvalidTag(u8),
    /// Not enough bytes in the buffer.
    BufferTooShort,
    /// String bytes are not valid UTF-8.
    InvalidUtf8,
    /// Field value is out of the valid range.
    InvalidData(String),
    /// String length exceeds u16::MAX (65535) bytes.
    StringTooLong(usize),
    /// List item count exceeds u8::MAX (255).
    TooManyItems(usize),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::InvalidTag(t) => write!(f, "invalid tag: 0x{t:02X}"),
            CodecError::BufferTooShort => write!(f, "buffer too short"),
            CodecError::InvalidUtf8 => write!(f, "invalid UTF-8 in string field"),
            CodecError::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            CodecError::StringTooLong(len) => {
                write!(f, "string too long: {len} bytes (max {})", u16::MAX)
            }
            CodecError::TooManyItems(count) => {
                write!(f, "too many items: {count} (max {})", u8::MAX)
            }
        }
    }
}

impl std::error::Error for CodecError {}

// ── Helpers ──

fn encode_string(buf: &mut Vec<u8>, s: &str) -> Result<(), CodecError> {
    if s.len() > u16::MAX as usize {
        return Err(CodecError::StringTooLong(s.len()));
    }
    let len = s.len() as u16;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    Ok(())
}

fn decode_string(data: &[u8], offset: &mut usize) -> Result<String, CodecError> {
    if *offset + 2 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let len = u16::from_le_bytes([data[*offset], data[*offset + 1]]) as usize;
    *offset += 2;
    if *offset + len > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let s = std::str::from_utf8(&data[*offset..*offset + len]).map_err(|_| CodecError::InvalidUtf8)?;
    *offset += len;
    Ok(s.to_owned())
}

fn encode_button_id(id: &ButtonId) -> u8 {
    match id {
        ButtonId::Delete => 0,
        ButtonId::Cancel => 1,
        ButtonId::Mode => 2,
        ButtonId::Session => 3,
        ButtonId::Send => 4,
        ButtonId::Voice => 5,
    }
}

fn decode_button_id(b: u8) -> Result<ButtonId, CodecError> {
    match b {
        0 => Ok(ButtonId::Delete),
        1 => Ok(ButtonId::Cancel),
        2 => Ok(ButtonId::Mode),
        3 => Ok(ButtonId::Session),
        4 => Ok(ButtonId::Send),
        5 => Ok(ButtonId::Voice),
        _ => Err(CodecError::InvalidData(format!("invalid button id: {b}"))),
    }
}

fn encode_direction(d: &Direction) -> u8 {
    match d {
        Direction::Clockwise => 0,
        Direction::CounterClockwise => 1,
    }
}

fn decode_direction(b: u8) -> Result<Direction, CodecError> {
    match b {
        0 => Ok(Direction::Clockwise),
        1 => Ok(Direction::CounterClockwise),
        _ => Err(CodecError::InvalidData(format!("invalid direction: {b}"))),
    }
}

fn encode_permission_action(a: &PermissionAction) -> u8 {
    match a {
        PermissionAction::Allow => 0,
        PermissionAction::Deny => 1,
        PermissionAction::Always => 2,
    }
}

fn decode_permission_action(b: u8) -> Result<PermissionAction, CodecError> {
    match b {
        0 => Ok(PermissionAction::Allow),
        1 => Ok(PermissionAction::Deny),
        2 => Ok(PermissionAction::Always),
        _ => Err(CodecError::InvalidData(format!("invalid permission action: {b}"))),
    }
}

fn encode_session_status(s: &SessionStatus) -> u8 {
    match s {
        SessionStatus::Thinking => 0,
        SessionStatus::ToolUse => 1,
        SessionStatus::Writing => 2,
        SessionStatus::Done => 3,
        SessionStatus::Error => 4,
        SessionStatus::Idle => 5,
        SessionStatus::PermissionNeeded => 6,
    }
}

fn decode_session_status(b: u8) -> Result<SessionStatus, CodecError> {
    match b {
        0 => Ok(SessionStatus::Thinking),
        1 => Ok(SessionStatus::ToolUse),
        2 => Ok(SessionStatus::Writing),
        3 => Ok(SessionStatus::Done),
        4 => Ok(SessionStatus::Error),
        5 => Ok(SessionStatus::Idle),
        6 => Ok(SessionStatus::PermissionNeeded),
        _ => Err(CodecError::InvalidData(format!("invalid session status: {b}"))),
    }
}

fn encode_sound_type(s: &SoundType) -> u8 {
    match s {
        SoundType::PermissionAlert => 0,
        SoundType::SessionComplete => 1,
        SoundType::Error => 2,
        SoundType::Click => 3,
    }
}

fn decode_sound_type(b: u8) -> Result<SoundType, CodecError> {
    match b {
        0 => Ok(SoundType::PermissionAlert),
        1 => Ok(SoundType::SessionComplete),
        2 => Ok(SoundType::Error),
        3 => Ok(SoundType::Click),
        _ => Err(CodecError::InvalidData(format!("invalid sound type: {b}"))),
    }
}

fn encode_led_color(buf: &mut Vec<u8>, c: &LedColor) {
    buf.push(c.r);
    buf.push(c.g);
    buf.push(c.b);
}

fn decode_led_color(data: &[u8], offset: &mut usize) -> Result<LedColor, CodecError> {
    if *offset + 3 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let c = LedColor {
        r: data[*offset],
        g: data[*offset + 1],
        b: data[*offset + 2],
    };
    *offset += 3;
    Ok(c)
}

fn encode_u64_le(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn decode_u64_le(data: &[u8], offset: &mut usize) -> Result<u64, CodecError> {
    if *offset + 8 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let v = u64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap());
    *offset += 8;
    Ok(v)
}

fn encode_f64_le(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn decode_f64_le(data: &[u8], offset: &mut usize) -> Result<f64, CodecError> {
    if *offset + 8 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let v = f64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap());
    *offset += 8;
    Ok(v)
}

fn encode_session_info(buf: &mut Vec<u8>, info: &SessionInfo) -> Result<(), CodecError> {
    // Basic
    buf.extend_from_slice(&info.id.to_le_bytes());
    encode_string(buf, &info.name)?;
    buf.push(encode_session_status(&info.status));
    buf.push(u8::from(info.has_permission_request));
    // Hook extensions
    encode_string(buf, &info.source)?;
    encode_string(buf, &info.cwd)?;
    encode_string(buf, &info.permission_mode)?;
    // JSONL
    encode_string(buf, &info.model)?;
    encode_u64_le(buf, info.tokens_in);
    encode_u64_le(buf, info.tokens_out);
    encode_f64_le(buf, info.cost_usd);
    buf.push(info.context_pct);
    encode_string(buf, &info.last_message)?;
    encode_string(buf, &info.last_ai_output)?;
    // Window
    encode_string(buf, &info.bundle_id)?;
    encode_string(buf, &info.session_tty)?;
    // Timing
    encode_u64_le(buf, info.started_at);
    encode_u64_le(buf, info.last_activity);
    Ok(())
}

fn decode_session_info(data: &[u8], offset: &mut usize) -> Result<SessionInfo, CodecError> {
    // Basic
    if *offset + 2 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let id = u16::from_le_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    let name = decode_string(data, offset)?;
    if *offset + 2 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let status = decode_session_status(data[*offset])?;
    *offset += 1;
    let has_permission_request = data[*offset] != 0;
    *offset += 1;
    // Hook extensions
    let source = decode_string(data, offset)?;
    let cwd = decode_string(data, offset)?;
    let permission_mode = decode_string(data, offset)?;
    // JSONL
    let model = decode_string(data, offset)?;
    let tokens_in = decode_u64_le(data, offset)?;
    let tokens_out = decode_u64_le(data, offset)?;
    let cost_usd = decode_f64_le(data, offset)?;
    if *offset >= data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let context_pct = data[*offset];
    *offset += 1;
    let last_message = decode_string(data, offset)?;
    let last_ai_output = decode_string(data, offset)?;
    // Window
    let bundle_id = decode_string(data, offset)?;
    let session_tty = decode_string(data, offset)?;
    // Timing
    let started_at = decode_u64_le(data, offset)?;
    let last_activity = decode_u64_le(data, offset)?;

    Ok(SessionInfo {
        id,
        name,
        status,
        has_permission_request,
        source,
        cwd,
        permission_mode,
        model,
        tokens_in,
        tokens_out,
        cost_usd,
        context_pct,
        last_message,
        last_ai_output,
        bundle_id,
        session_tty,
        started_at,
        last_activity,
    })
}

fn encode_notification_info(buf: &mut Vec<u8>, info: &NotificationInfo) -> Result<(), CodecError> {
    buf.extend_from_slice(&info.id.to_le_bytes());
    buf.extend_from_slice(&info.session_id.to_le_bytes());
    encode_string(buf, &info.session_name)?;
    buf.push(encode_session_status(&info.status));
    encode_string(buf, &info.description)?;
    encode_u64_le(buf, info.timestamp);
    buf.push(u8::from(info.read));
    Ok(())
}

fn decode_notification_info(data: &[u8], offset: &mut usize) -> Result<NotificationInfo, CodecError> {
    if *offset + 4 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let id = u32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    if *offset + 2 > data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let session_id = u16::from_le_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    let session_name = decode_string(data, offset)?;
    if *offset >= data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let status = decode_session_status(data[*offset])?;
    *offset += 1;
    let description = decode_string(data, offset)?;
    let timestamp = decode_u64_le(data, offset)?;
    if *offset >= data.len() {
        return Err(CodecError::BufferTooShort);
    }
    let read = data[*offset] != 0;
    *offset += 1;
    Ok(NotificationInfo {
        id,
        session_id,
        session_name,
        status,
        description,
        timestamp,
        read,
    })
}

// ── Public API ──

/// Encode an uplink message to bytes.
pub fn encode_uplink(msg: &UplinkMessage) -> Result<Vec<u8>, CodecError> {
    let mut buf = Vec::new();
    match msg {
        UplinkMessage::ButtonPress(id) => {
            buf.push(TAG_BUTTON_PRESS);
            buf.push(encode_button_id(id));
        }
        UplinkMessage::ButtonRelease(id) => {
            buf.push(TAG_BUTTON_RELEASE);
            buf.push(encode_button_id(id));
        }
        UplinkMessage::KnobRotate { direction, steps } => {
            buf.push(TAG_KNOB_ROTATE);
            buf.push(encode_direction(direction));
            buf.push(*steps);
        }
        UplinkMessage::KnobPress => {
            buf.push(TAG_KNOB_PRESS);
        }
        UplinkMessage::KnobRelease => {
            buf.push(TAG_KNOB_RELEASE);
        }
        UplinkMessage::PermissionResponse { session_id, action } => {
            buf.push(TAG_PERMISSION_RESPONSE);
            buf.extend_from_slice(&session_id.to_le_bytes());
            buf.push(encode_permission_action(action));
        }
        UplinkMessage::SessionSwitch { session_id } => {
            buf.push(TAG_SESSION_SWITCH);
            buf.extend_from_slice(&session_id.to_le_bytes());
        }
    }
    Ok(buf)
}

/// Decode an uplink message from bytes.
pub fn decode_uplink(data: &[u8]) -> Result<UplinkMessage, CodecError> {
    if data.is_empty() {
        return Err(CodecError::BufferTooShort);
    }
    let tag = data[0];
    let rest = &data[1..];
    match tag {
        TAG_BUTTON_PRESS => {
            if rest.is_empty() {
                return Err(CodecError::BufferTooShort);
            }
            Ok(UplinkMessage::ButtonPress(decode_button_id(rest[0])?))
        }
        TAG_BUTTON_RELEASE => {
            if rest.is_empty() {
                return Err(CodecError::BufferTooShort);
            }
            Ok(UplinkMessage::ButtonRelease(decode_button_id(rest[0])?))
        }
        TAG_KNOB_ROTATE => {
            if rest.len() < 2 {
                return Err(CodecError::BufferTooShort);
            }
            Ok(UplinkMessage::KnobRotate {
                direction: decode_direction(rest[0])?,
                steps: rest[1],
            })
        }
        TAG_KNOB_PRESS => Ok(UplinkMessage::KnobPress),
        TAG_KNOB_RELEASE => Ok(UplinkMessage::KnobRelease),
        TAG_PERMISSION_RESPONSE => {
            if rest.len() < 3 {
                return Err(CodecError::BufferTooShort);
            }
            let session_id = u16::from_le_bytes([rest[0], rest[1]]);
            let action = decode_permission_action(rest[2])?;
            Ok(UplinkMessage::PermissionResponse { session_id, action })
        }
        TAG_SESSION_SWITCH => {
            if rest.len() < 2 {
                return Err(CodecError::BufferTooShort);
            }
            let session_id = u16::from_le_bytes([rest[0], rest[1]]);
            Ok(UplinkMessage::SessionSwitch { session_id })
        }
        _ => Err(CodecError::InvalidTag(tag)),
    }
}

/// Encode a downlink message to bytes.
pub fn encode_downlink(msg: &DownlinkMessage) -> Result<Vec<u8>, CodecError> {
    let mut buf = Vec::new();
    match msg {
        DownlinkMessage::SessionListUpdate {
            sessions,
            active_index,
        } => {
            buf.push(TAG_SESSION_LIST_UPDATE);
            if sessions.len() > u8::MAX as usize {
                return Err(CodecError::TooManyItems(sessions.len()));
            }
            buf.push(sessions.len() as u8);
            for s in sessions {
                encode_session_info(&mut buf, s)?;
            }
            buf.push(*active_index);
        }
        DownlinkMessage::SessionStatusChange { session_id, status } => {
            buf.push(TAG_SESSION_STATUS_CHANGE);
            buf.extend_from_slice(&session_id.to_le_bytes());
            buf.push(encode_session_status(status));
        }
        DownlinkMessage::PermissionRequest {
            session_id,
            action_desc,
        } => {
            buf.push(TAG_PERMISSION_REQUEST);
            buf.extend_from_slice(&session_id.to_le_bytes());
            encode_string(&mut buf, action_desc)?;
        }
        DownlinkMessage::SetLed {
            button,
            color,
            blink,
        } => {
            buf.push(TAG_SET_LED);
            buf.push(encode_button_id(button));
            encode_led_color(&mut buf, color);
            buf.push(u8::from(*blink));
        }
        DownlinkMessage::SetKnobRing(color) => {
            buf.push(TAG_SET_KNOB_RING);
            encode_led_color(&mut buf, color);
        }
        DownlinkMessage::PlaySound(sound) => {
            buf.push(TAG_PLAY_SOUND);
            buf.push(encode_sound_type(sound));
        }
        DownlinkMessage::DismissPermission { session_id } => {
            buf.push(TAG_DISMISS_PERMISSION);
            buf.extend_from_slice(&session_id.to_le_bytes());
        }
        DownlinkMessage::NotificationListUpdate { notifications } => {
            buf.push(TAG_NOTIFICATION_LIST_UPDATE);
            if notifications.len() > u8::MAX as usize {
                return Err(CodecError::TooManyItems(notifications.len()));
            }
            buf.push(notifications.len() as u8);
            for n in notifications {
                encode_notification_info(&mut buf, n)?;
            }
        }
        DownlinkMessage::FrameData { width, height, pixels } => {
            buf.push(TAG_FRAME_DATA);
            buf.extend_from_slice(&width.to_le_bytes());
            buf.extend_from_slice(&height.to_le_bytes());
            let len = pixels.len() as u32;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(pixels);
        }
        DownlinkMessage::SetVolume(vol) => {
            buf.push(TAG_SET_VOLUME);
            buf.push(*vol);
        }
        DownlinkMessage::SetMuted(muted) => {
            buf.push(TAG_SET_MUTED);
            buf.push(u8::from(*muted));
        }
        DownlinkMessage::SetSoundMapping { sound_type, sound_id } => {
            buf.push(TAG_SET_SOUND_MAPPING);
            buf.push(encode_sound_type(sound_type));
            encode_string(&mut buf, sound_id)?;
        }
    }
    Ok(buf)
}

/// Decode a downlink message from bytes.
pub fn decode_downlink(data: &[u8]) -> Result<DownlinkMessage, CodecError> {
    if data.is_empty() {
        return Err(CodecError::BufferTooShort);
    }
    let tag = data[0];
    let mut offset = 1;
    match tag {
        TAG_SESSION_LIST_UPDATE => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let count = data[offset] as usize;
            offset += 1;
            let mut sessions = Vec::with_capacity(count);
            for _ in 0..count {
                sessions.push(decode_session_info(data, &mut offset)?);
            }
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let active_index = data[offset];
            Ok(DownlinkMessage::SessionListUpdate {
                sessions,
                active_index,
            })
        }
        TAG_SESSION_STATUS_CHANGE => {
            if offset + 3 > data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let session_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
            let status = decode_session_status(data[offset])?;
            Ok(DownlinkMessage::SessionStatusChange { session_id, status })
        }
        TAG_PERMISSION_REQUEST => {
            if offset + 2 > data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let session_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
            let action_desc = decode_string(data, &mut offset)?;
            Ok(DownlinkMessage::PermissionRequest {
                session_id,
                action_desc,
            })
        }
        TAG_SET_LED => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let button = decode_button_id(data[offset])?;
            offset += 1;
            let color = decode_led_color(data, &mut offset)?;
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let blink = data[offset] != 0;
            Ok(DownlinkMessage::SetLed {
                button,
                color,
                blink,
            })
        }
        TAG_SET_KNOB_RING => {
            let color = decode_led_color(data, &mut offset)?;
            Ok(DownlinkMessage::SetKnobRing(color))
        }
        TAG_PLAY_SOUND => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            Ok(DownlinkMessage::PlaySound(decode_sound_type(data[offset])?))
        }
        TAG_DISMISS_PERMISSION => {
            if offset + 2 > data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let session_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
            Ok(DownlinkMessage::DismissPermission { session_id })
        }
        TAG_FRAME_DATA => {
            if offset + 8 > data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let width = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
            let height = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
            let pixel_len = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
            offset += 4;
            if offset + pixel_len > data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let pixels = data[offset..offset + pixel_len].to_vec();
            Ok(DownlinkMessage::FrameData { width, height, pixels })
        }
        TAG_NOTIFICATION_LIST_UPDATE => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let count = data[offset] as usize;
            offset += 1;
            let mut notifications = Vec::with_capacity(count);
            for _ in 0..count {
                notifications.push(decode_notification_info(data, &mut offset)?);
            }
            Ok(DownlinkMessage::NotificationListUpdate { notifications })
        }
        TAG_SET_VOLUME => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            Ok(DownlinkMessage::SetVolume(data[offset]))
        }
        TAG_SET_MUTED => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            Ok(DownlinkMessage::SetMuted(data[offset] != 0))
        }
        TAG_SET_SOUND_MAPPING => {
            if offset >= data.len() {
                return Err(CodecError::BufferTooShort);
            }
            let sound_type = decode_sound_type(data[offset])?;
            offset += 1;
            let sound_id = decode_string(data, &mut offset)?;
            Ok(DownlinkMessage::SetSoundMapping { sound_type, sound_id })
        }
        _ => Err(CodecError::InvalidTag(tag)),
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_uplink_roundtrip(msg: &UplinkMessage) {
        let encoded = encode_uplink(msg).unwrap();
        let decoded = decode_uplink(&encoded).unwrap();
        assert_eq!(format!("{decoded:?}"), format!("{msg:?}"));
    }

    fn assert_downlink_roundtrip(msg: &DownlinkMessage) {
        let encoded = encode_downlink(msg).unwrap();
        let decoded = decode_downlink(&encoded).unwrap();
        assert_eq!(format!("{decoded:?}"), format!("{msg:?}"));
    }

    #[test]
    fn roundtrip_button_press() {
        for id in [
            ButtonId::Delete,
            ButtonId::Cancel,
            ButtonId::Mode,
            ButtonId::Session,
            ButtonId::Send,
            ButtonId::Voice,
        ] {
            assert_uplink_roundtrip(&UplinkMessage::ButtonPress(id));
        }
    }

    #[test]
    fn roundtrip_button_release() {
        for id in [
            ButtonId::Delete,
            ButtonId::Cancel,
            ButtonId::Mode,
            ButtonId::Session,
            ButtonId::Send,
            ButtonId::Voice,
        ] {
            assert_uplink_roundtrip(&UplinkMessage::ButtonRelease(id));
        }
    }

    #[test]
    fn roundtrip_knob_rotate() {
        assert_uplink_roundtrip(&UplinkMessage::KnobRotate {
            direction: Direction::Clockwise,
            steps: 3,
        });
        assert_uplink_roundtrip(&UplinkMessage::KnobRotate {
            direction: Direction::CounterClockwise,
            steps: 255,
        });
    }

    #[test]
    fn roundtrip_knob_press_release() {
        assert_uplink_roundtrip(&UplinkMessage::KnobPress);
        assert_uplink_roundtrip(&UplinkMessage::KnobRelease);
    }

    #[test]
    fn roundtrip_permission_response() {
        for action in [
            PermissionAction::Allow,
            PermissionAction::Deny,
            PermissionAction::Always,
        ] {
            assert_uplink_roundtrip(&UplinkMessage::PermissionResponse {
                session_id: 42,
                action,
            });
        }
    }

    #[test]
    fn roundtrip_session_switch() {
        assert_uplink_roundtrip(&UplinkMessage::SessionSwitch { session_id: 0 });
        assert_uplink_roundtrip(&UplinkMessage::SessionSwitch { session_id: 65535 });
    }

    #[test]
    fn roundtrip_session_list_update() {
        let msg = DownlinkMessage::SessionListUpdate {
            sessions: vec![
                SessionInfo {
                    id: 1,
                    name: "Claude".to_string(),
                    status: SessionStatus::Thinking,
                    has_permission_request: false,
                    ..Default::default()
                },
                SessionInfo {
                    id: 2,
                    name: "GPT".to_string(),
                    status: SessionStatus::PermissionNeeded,
                    has_permission_request: true,
                    ..Default::default()
                },
            ],
            active_index: 0,
        };
        assert_downlink_roundtrip(&msg);
    }

    #[test]
    fn roundtrip_session_list_empty() {
        let msg = DownlinkMessage::SessionListUpdate {
            sessions: vec![],
            active_index: 0,
        };
        assert_downlink_roundtrip(&msg);
    }

    #[test]
    fn roundtrip_session_status_change() {
        for status in [
            SessionStatus::Thinking,
            SessionStatus::ToolUse,
            SessionStatus::Writing,
            SessionStatus::Done,
            SessionStatus::Error,
            SessionStatus::Idle,
            SessionStatus::PermissionNeeded,
        ] {
            assert_downlink_roundtrip(&DownlinkMessage::SessionStatusChange {
                session_id: 100,
                status,
            });
        }
    }

    #[test]
    fn roundtrip_permission_request() {
        assert_downlink_roundtrip(&DownlinkMessage::PermissionRequest {
            session_id: 7,
            action_desc: "rm -rf /".to_string(),
        });
    }

    #[test]
    fn roundtrip_permission_request_unicode() {
        assert_downlink_roundtrip(&DownlinkMessage::PermissionRequest {
            session_id: 7,
            action_desc: "删除文件 /tmp/test".to_string(),
        });
    }

    #[test]
    fn roundtrip_set_led() {
        assert_downlink_roundtrip(&DownlinkMessage::SetLed {
            button: ButtonId::Send,
            color: LedColor::GREEN,
            blink: true,
        });
        assert_downlink_roundtrip(&DownlinkMessage::SetLed {
            button: ButtonId::Delete,
            color: LedColor::OFF,
            blink: false,
        });
    }

    #[test]
    fn roundtrip_set_knob_ring() {
        assert_downlink_roundtrip(&DownlinkMessage::SetKnobRing(LedColor::AMBER));
    }

    #[test]
    fn roundtrip_play_sound() {
        for sound in [
            SoundType::PermissionAlert,
            SoundType::SessionComplete,
            SoundType::Error,
            SoundType::Click,
        ] {
            assert_downlink_roundtrip(&DownlinkMessage::PlaySound(sound));
        }
    }

    #[test]
    fn roundtrip_dismiss_permission() {
        assert_downlink_roundtrip(&DownlinkMessage::DismissPermission { session_id: 42 });
    }

    #[test]
    fn error_empty_buffer() {
        assert_eq!(decode_uplink(&[]), Err(CodecError::BufferTooShort));
        assert_eq!(decode_downlink(&[]), Err(CodecError::BufferTooShort));
    }

    #[test]
    fn error_invalid_tag() {
        assert_eq!(decode_uplink(&[0xFF]), Err(CodecError::InvalidTag(0xFF)));
        assert_eq!(decode_downlink(&[0x00]), Err(CodecError::InvalidTag(0x00)));
    }

    #[test]
    fn error_truncated_uplink() {
        // ButtonPress needs 1 byte after tag
        assert_eq!(decode_uplink(&[TAG_BUTTON_PRESS]), Err(CodecError::BufferTooShort));
        // KnobRotate needs 2 bytes after tag
        assert_eq!(
            decode_uplink(&[TAG_KNOB_ROTATE, 0x00]),
            Err(CodecError::BufferTooShort)
        );
    }

    #[test]
    fn error_invalid_button_id() {
        assert!(matches!(
            decode_uplink(&[TAG_BUTTON_PRESS, 0xFF]),
            Err(CodecError::InvalidData(_))
        ));
    }

    #[test]
    fn error_invalid_utf8() {
        // PermissionRequest with invalid UTF-8 string
        let mut data = vec![TAG_PERMISSION_REQUEST, 0x01, 0x00]; // session_id = 1
        data.extend_from_slice(&[0x02, 0x00]); // string length = 2
        data.extend_from_slice(&[0xFF, 0xFE]); // invalid UTF-8
        assert_eq!(decode_downlink(&data), Err(CodecError::InvalidUtf8));
    }

    #[test]
    fn tag_values_uplink() {
        assert_eq!(encode_uplink(&UplinkMessage::ButtonPress(ButtonId::Delete)).unwrap()[0], 0x01);
        assert_eq!(encode_uplink(&UplinkMessage::ButtonRelease(ButtonId::Delete)).unwrap()[0], 0x02);
        assert_eq!(
            encode_uplink(&UplinkMessage::KnobRotate {
                direction: Direction::Clockwise,
                steps: 1,
            }).unwrap()[0],
            0x03
        );
        assert_eq!(encode_uplink(&UplinkMessage::KnobPress).unwrap()[0], 0x04);
        assert_eq!(encode_uplink(&UplinkMessage::KnobRelease).unwrap()[0], 0x05);
        assert_eq!(
            encode_uplink(&UplinkMessage::PermissionResponse {
                session_id: 0,
                action: PermissionAction::Allow,
            }).unwrap()[0],
            0x06
        );
        assert_eq!(
            encode_uplink(&UplinkMessage::SessionSwitch { session_id: 0 }).unwrap()[0],
            0x07
        );
    }

    #[test]
    fn tag_values_downlink() {
        assert_eq!(
            encode_downlink(&DownlinkMessage::SessionListUpdate {
                sessions: vec![],
                active_index: 0,
            }).unwrap()[0],
            0x81
        );
        assert_eq!(
            encode_downlink(&DownlinkMessage::SessionStatusChange {
                session_id: 0,
                status: SessionStatus::Idle,
            }).unwrap()[0],
            0x82
        );
        assert_eq!(
            encode_downlink(&DownlinkMessage::PermissionRequest {
                session_id: 0,
                action_desc: String::new(),
            }).unwrap()[0],
            0x83
        );
        assert_eq!(
            encode_downlink(&DownlinkMessage::SetLed {
                button: ButtonId::Send,
                color: LedColor::OFF,
                blink: false,
            }).unwrap()[0],
            0x84
        );
        assert_eq!(encode_downlink(&DownlinkMessage::SetKnobRing(LedColor::OFF)).unwrap()[0], 0x85);
        assert_eq!(
            encode_downlink(&DownlinkMessage::PlaySound(SoundType::Click)).unwrap()[0],
            0x86
        );
        assert_eq!(encode_downlink(&DownlinkMessage::DismissPermission { session_id: 0 }).unwrap()[0], 0x87);
        assert_eq!(
            encode_downlink(&DownlinkMessage::NotificationListUpdate {
                notifications: vec![],
            }).unwrap()[0],
            0x89
        );
    }

    #[test]
    fn roundtrip_notification_list_update() {
        let msg = DownlinkMessage::NotificationListUpdate {
            notifications: vec![
                NotificationInfo {
                    id: 42,
                    session_id: 1,
                    session_name: "Claude Code".to_string(),
                    status: SessionStatus::Done,
                    description: "Task completed successfully".to_string(),
                    timestamp: 1717171717,
                    read: false,
                },
                NotificationInfo {
                    id: 43,
                    session_id: 2,
                    session_name: "Codex".to_string(),
                    status: SessionStatus::Error,
                    description: "Build failed".to_string(),
                    timestamp: 1717171800,
                    read: true,
                },
            ],
        };
        assert_downlink_roundtrip(&msg);
    }

    #[test]
    fn roundtrip_notification_list_empty() {
        let msg = DownlinkMessage::NotificationListUpdate {
            notifications: vec![],
        };
        assert_downlink_roundtrip(&msg);
    }

    #[test]
    fn roundtrip_set_volume() {
        assert_downlink_roundtrip(&DownlinkMessage::SetVolume(0));
        assert_downlink_roundtrip(&DownlinkMessage::SetVolume(80));
        assert_downlink_roundtrip(&DownlinkMessage::SetVolume(100));
    }

    #[test]
    fn roundtrip_set_muted() {
        assert_downlink_roundtrip(&DownlinkMessage::SetMuted(true));
        assert_downlink_roundtrip(&DownlinkMessage::SetMuted(false));
    }

    #[test]
    fn roundtrip_set_sound_mapping() {
        for sound_type in [
            SoundType::PermissionAlert,
            SoundType::SessionComplete,
            SoundType::Error,
            SoundType::Click,
        ] {
            assert_downlink_roundtrip(&DownlinkMessage::SetSoundMapping {
                sound_type,
                sound_id: "builtin:alert".to_string(),
            });
        }
        assert_downlink_roundtrip(&DownlinkMessage::SetSoundMapping {
            sound_type: SoundType::Click,
            sound_id: "custom:my_sound".to_string(),
        });
    }

    #[test]
    fn tag_values_new_downlink() {
        assert_eq!(encode_downlink(&DownlinkMessage::SetVolume(50)).unwrap()[0], 0x8A);
        assert_eq!(encode_downlink(&DownlinkMessage::SetMuted(true)).unwrap()[0], 0x8B);
        assert_eq!(
            encode_downlink(&DownlinkMessage::SetSoundMapping {
                sound_type: SoundType::Click,
                sound_id: "builtin:click".to_string(),
            }).unwrap()[0],
            0x8C
        );
    }

    #[test]
    fn error_string_too_long() {
        let long_string = "x".repeat(u16::MAX as usize + 1);
        let msg = DownlinkMessage::PermissionRequest {
            session_id: 1,
            action_desc: long_string,
        };
        assert!(matches!(
            encode_downlink(&msg),
            Err(CodecError::StringTooLong(_))
        ));
    }

    #[test]
    fn error_too_many_sessions() {
        let sessions: Vec<SessionInfo> = (0..256)
            .map(|i| SessionInfo {
                id: i as u16,
                name: format!("s{i}"),
                ..Default::default()
            })
            .collect();
        let msg = DownlinkMessage::SessionListUpdate {
            sessions,
            active_index: 0,
        };
        assert!(matches!(
            encode_downlink(&msg),
            Err(CodecError::TooManyItems(256))
        ));
    }

    #[test]
    fn error_too_many_notifications() {
        let notifications: Vec<NotificationInfo> = (0..256)
            .map(|i| NotificationInfo {
                id: i as u32,
                session_id: 0,
                session_name: String::new(),
                status: SessionStatus::Idle,
                description: String::new(),
                timestamp: 0,
                read: false,
            })
            .collect();
        let msg = DownlinkMessage::NotificationListUpdate { notifications };
        assert!(matches!(
            encode_downlink(&msg),
            Err(CodecError::TooManyItems(256))
        ));
    }
}
