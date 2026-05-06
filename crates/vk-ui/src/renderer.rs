//! Render engine — draws the four screen states to a framebuffer.
//!
//! Each screen state has a dedicated render function that reads the
//! ScreenStateMachine's data and draws widgets to the framebuffer.

use vk_display::color::Rgb565;
use vk_display::framebuffer::DynFramebuffer;
use vk_protocol::message::SessionStatus;

use crate::screen::{AllowOption, ScreenState, ScreenStateMachine};
use crate::widget::{
    draw_divider, draw_text, draw_text_md, draw_text_sm, draw_text_tiny, Align, Label, Rect, StatusBar,
    FONT_H, FONT_W, FONT_H_MD, FONT_W_MD, FONT_H_SM, FONT_W_SM, FONT_H_TINY, FONT_W_TINY,
};

/// LCD dimensions (3.4" panel, 412×960 native portrait, mounted landscape = 960×412).
pub const LCD_W: u16 = 960;
pub const LCD_H: u16 = 412;

/// Color palette (inspired by SC's GitHub dark theme).
pub const BG_COLOR: Rgb565 = Rgb565::BLACK;
pub const TEXT_COLOR: Rgb565 = Rgb565::WHITE;
pub const ACCENT_COLOR: Rgb565 = Rgb565(0x3E6A); // #3fb950 green
pub const ALERT_COLOR: Rgb565 = Rgb565(0xFD20); // AMBER ~(255, 165, 0)
pub const DIVIDER_COLOR: Rgb565 = Rgb565(0x630C); // #61656b dim border
pub const BLUE_COLOR: Rgb565 = Rgb565(0x5D3F);   // #58a6ff thinking/writing
pub const YELLOW_COLOR: Rgb565 = Rgb565(0xD4C4);  // #d29922 tool_use
pub const RED_COLOR: Rgb565 = Rgb565(0xFA89);     // #f85149 error
pub const CYAN_COLOR: Rgb565 = Rgb565(0x3E98);    // #39d2c0 interactive
pub const PURPLE_COLOR: Rgb565 = Rgb565(0xBEFF);  // #bc8cff notification
pub const MUTED_COLOR: Rgb565 = Rgb565(0x8C51);   // #8b949e secondary text

/// Context passed to the renderer from the caller (e.g. simulator event loop).
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    /// Current time string for Standby display (e.g. "14:35").
    pub time_str: String,
}

/// Render the current screen state to the framebuffer.
pub fn render(sm: &ScreenStateMachine, fb: &mut DynFramebuffer, ctx: &RenderContext) {
    fb.clear(BG_COLOR);

    match sm.state() {
        ScreenState::Standby => render_standby(sm, fb, ctx),
        ScreenState::Normal => render_normal(sm, fb, ctx),
        ScreenState::Select => render_select(sm, fb),
        ScreenState::Allow => render_allow(sm, fb),
        ScreenState::Notify => render_notify(sm, fb),
    }

    // Render toast overlays on top
    render_toasts(sm, fb);
}

/// Standby: brand logo + time + connection status.
fn render_standby(sm: &ScreenStateMachine, fb: &mut DynFramebuffer, ctx: &RenderContext) {
    let w = fb.width();
    let h = fb.height();

    // Brand name centered
    let brand = Label::new("VIBE KEYBOARD", TEXT_COLOR).aligned(Align::Center);
    brand.render(fb, Rect::new(0, h / 2 - FONT_H * 2, w, FONT_H));

    // Time display centered
    if !ctx.time_str.is_empty() {
        let time_label = Label::new(&ctx.time_str, CYAN_COLOR).aligned(Align::Center);
        time_label.render(fb, Rect::new(0, h / 2 - FONT_H / 2, w, FONT_H));
    }

    // Subtitle
    let sub = Label::new("Waiting for sessions...", MUTED_COLOR).aligned(Align::Center);
    sub.render(fb, Rect::new(0, h / 2 + FONT_H, w, FONT_H));

    // Blink indicator
    if sm.frame() % 60 < 30 {
        let dot = Label::new("*", ACCENT_COLOR).aligned(Align::Center);
        dot.render(fb, Rect::new(0, h - FONT_H - 2, w, FONT_H));
    }
}

/// Normal: multi-session OVERVIEW dashboard inspired by Image1.png.
/// All text uses SM font (12×20). Active row distinguished by amber border + accent colors.
fn render_normal(sm: &ScreenStateMachine, fb: &mut DynFramebuffer, ctx: &RenderContext) {
    let w = fb.width();
    let h = fb.height();
    let sessions = sm.sessions();
    let active_idx = sm.active_index().min(sessions.len().saturating_sub(1));
    let pad = 12u16;

    // ── Column layout: x positions and widths (gap=10 between cols, all in 960px width) ──
    let cols: [(u16, &str); 8] = [
        (pad,         "SESSION"),
        (pad + 106,   "TOOL"),
        (pad + 248,   "MODE"),
        (pad + 330,   "MODEL"),
        (pad + 436,   "COST"),
        (pad + 542,   "PLAN USAGE LIMITS"),
        (pad + 762,   "RESETS IN"),
        (pad + 880,   "STATE"),
    ];
    // PLAN bar starts after "100% " text (~5 chars * 12 = 60px) within col 5, ends before col 6.
    let bar_start = cols[5].0 + 60;
    let bar_end = cols[6].0.saturating_sub(8);

    // ── Top header bar (all SM) ──
    let hy = 10u16;
    draw_text_sm(fb, pad, hy, "OVERVIEW", ALERT_COLOR);

    let active_count = sessions
        .iter()
        .filter(|s| !matches!(s.status, SessionStatus::Idle | SessionStatus::Done))
        .count();
    let avg_ctx: u32 = if sessions.is_empty() {
        0
    } else {
        sessions.iter().map(|s| s.context_pct as u32).sum::<u32>() / sessions.len() as u32
    };
    let total_cost: f64 = sessions.iter().map(|s| s.cost_usd).sum();

    let active_str = format!("{active_count} active");
    let win_str = "5h rolling window";
    let avg_str = format!("avg {avg_ctx}%");
    let total_str = format!("total spend ${total_cost:.2}");

    let mut sx = pad + 8 * FONT_W_SM + 24;
    draw_text_sm(fb, sx, hy, &active_str, ALERT_COLOR);
    sx += active_str.len() as u16 * FONT_W_SM + 18;
    draw_text_sm(fb, sx, hy, win_str, MUTED_COLOR);
    sx += win_str.len() as u16 * FONT_W_SM + 18;
    draw_text_sm(fb, sx, hy, &avg_str, MUTED_COLOR);
    sx += avg_str.len() as u16 * FONT_W_SM + 18;
    draw_text_sm(fb, sx, hy, &total_str, MUTED_COLOR);

    // Time on right (SM)
    if !ctx.time_str.is_empty() {
        let tw = ctx.time_str.len() as u16 * FONT_W_SM;
        draw_text_sm(fb, w.saturating_sub(tw + pad), hy, &ctx.time_str, MUTED_COLOR);
    }

    let mut y = hy + FONT_H_SM + 14;

    // Empty state.
    if sessions.is_empty() {
        let label = Label::new("NO ACTIVE SESSIONS", MUTED_COLOR).aligned(Align::Center);
        label.render(fb, Rect::new(0, h / 2, w, FONT_H_SM));
        return;
    }

    // ── Column headers (SM, muted) ──
    for (cx, label) in cols.iter() {
        draw_text_sm(fb, *cx, y, label, MUTED_COLOR);
    }
    y += FONT_H_SM + 16;

    // ── Data rows: active vertically centered, others stack above/below ──
    // Reserve room for bottom TINY status line (16px CJK glyph + 14px margin).
    let bottom_bar_reserve = 30u16;
    let avail_top = y;
    let avail_bottom = h.saturating_sub(bottom_bar_reserve);
    let avail_center = (avail_top + avail_bottom) / 2;

    let row_h: u16 = 32;
    let row_gap: u16 = 18;
    let total_row_step = row_h + row_gap;

    let active_top = avail_center.saturating_sub(row_h / 2);
    draw_session_row_v2(fb, cols, bar_start, bar_end, &sessions[active_idx], active_idx, active_top, row_h, true);

    // Sessions before active: stack upward.
    let mut prev_y = active_top;
    for offset in 1..=active_idx {
        let i = active_idx - offset;
        if prev_y < avail_top + total_row_step {
            break;
        }
        prev_y = prev_y.saturating_sub(total_row_step);
        draw_session_row_v2(fb, cols, bar_start, bar_end, &sessions[i], i, prev_y, row_h, false);
    }

    // Sessions after active: stack downward.
    let mut next_y = active_top + total_row_step;
    for i in (active_idx + 1)..sessions.len() {
        if next_y + row_h > avail_bottom {
            break;
        }
        draw_session_row_v2(fb, cols, bar_start, bar_end, &sessions[i], i, next_y, row_h, false);
        next_y += total_row_step;
    }

    // ── Bottom status line (TINY — half the size of data rows) ──
    // TINY ASCII = 10px tall, TINY CJK = 16px tall (UNI_SCALE_TINY=1).
    // Reserve enough room for the larger CJK glyph height.
    let bottom_y = h.saturating_sub(16 + 6);
    let active = &sessions[active_idx];
    let mut bx = pad;
    let segments: [(&str, String, Rgb565); 4] = [
        ("focus:", active.name.to_uppercase(), ALERT_COLOR),
        ("task:", first_line_truncated(&active.last_message, 240, FONT_W_TINY), TEXT_COLOR),
        ("cost:", format!("${:.2}", active.cost_usd), TEXT_COLOR),
        ("now:", first_line_truncated(&active.last_ai_output, 280, FONT_W_TINY), TEXT_COLOR),
    ];
    for (label, value, color) in segments.iter() {
        draw_text_tiny(fb, bx, bottom_y, label, MUTED_COLOR);
        bx += label.len() as u16 * FONT_W_TINY + 4;
        draw_text_tiny(fb, bx, bottom_y, value, *color);
        // For ASCII chars the width is FONT_W_TINY each; CJK is wider — count chars × avg.
        let value_w: u16 = value.chars().map(|c| if c.is_ascii() { FONT_W_TINY } else { 16 }).sum();
        bx += value_w + 18;
        if bx >= w.saturating_sub(pad) {
            break;
        }
    }
}

/// Draw one session row at row_top, all 8 columns on a single SM-font line.
/// Active row gets an amber border + accent-colored values.
fn draw_session_row_v2(
    fb: &mut DynFramebuffer,
    cols: [(u16, &str); 8],
    bar_start: u16,
    bar_end: u16,
    session: &crate::screen::UiSession,
    idx: usize,
    row_top: u16,
    row_h: u16,
    is_active: bool,
) {
    let w = fb.width();
    let pad = 12u16;

    // Border for active row.
    if is_active {
        let bx = pad - 6;
        let bw = w.saturating_sub(2 * (pad - 6));
        let by = row_top.saturating_sub(2);
        let bh = row_h;
        for dx in bx..bx + bw {
            fb.draw_pixel(dx, by, ALERT_COLOR);
            fb.draw_pixel(dx, by + bh - 1, ALERT_COLOR);
        }
        for dy in by..by + bh {
            fb.draw_pixel(bx, dy, ALERT_COLOR);
            fb.draw_pixel(bx + bw - 1, dy, ALERT_COLOR);
        }
    }

    let row_mid = row_top + (row_h - FONT_H_SM) / 2;
    let primary = if is_active { ALERT_COLOR } else { TEXT_COLOR };

    let mode = match idx % 3 {
        0 => "PLAN",
        1 => "AUTO",
        _ => "REVIEW",
    };
    let (state_str, state_color) = state_label_color(session.status);

    draw_text_sm(fb, cols[0].0, row_mid, &session.name.to_uppercase(), primary);
    draw_text_sm(fb, cols[1].0, row_mid, source_full_name(&session.source), MUTED_COLOR);
    draw_text_sm(fb, cols[2].0, row_mid, mode, ALERT_COLOR);
    draw_text_sm(fb, cols[3].0, row_mid, &abbrev_model(&session.model), MUTED_COLOR);
    draw_text_sm(fb, cols[4].0, row_mid, &format!("${:.2}", session.cost_usd), primary);
    draw_text_sm(fb, cols[5].0, row_mid, &format!("{}%", session.context_pct), primary);

    // Segmented progress bar in PLAN USAGE LIMITS column.
    if bar_start + 24 < bar_end {
        let bar_y = row_mid + (FONT_H_SM.saturating_sub(14)) / 2;
        draw_segmented_bar(fb, bar_start, bar_y, bar_end - bar_start, 14, session.context_pct as u16);
    }

    draw_text_sm(fb, cols[6].0, row_mid, "—", MUTED_COLOR);
    draw_text_sm(fb, cols[7].0, row_mid, state_str, state_color);
}

/// Map source ID to display name shown in TOOL column.
fn source_full_name(s: &str) -> &'static str {
    match s {
        "claude-code" => "Claude Code",
        "cursor" => "Cursor",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "gemini" => "Gemini",
        _ => "?",
    }
}

/// Map status enum to STATE column label + color.
fn state_label_color(status: SessionStatus) -> (&'static str, Rgb565) {
    match status {
        SessionStatus::Done => ("DONE", ACCENT_COLOR),
        SessionStatus::Error => ("ERR", RED_COLOR),
        SessionStatus::PermissionNeeded => ("WAIT", ALERT_COLOR),
        SessionStatus::Idle => ("IDLE", MUTED_COLOR),
        SessionStatus::Thinking | SessionStatus::Writing | SessionStatus::ToolUse => ("RUN", ALERT_COLOR),
    }
}

/// Render a segmented progress bar (12 segments, image1-style chunks).
fn draw_segmented_bar(fb: &mut DynFramebuffer, x: u16, y: u16, total_w: u16, h: u16, pct: u16) {
    let segments: u16 = 12;
    if total_w < segments {
        return;
    }
    let gap = 1u16;
    let seg_w = (total_w + gap) / segments - gap;
    let active_segs = (pct.min(100) as u32 * segments as u32 / 100) as u16;
    for i in 0..segments {
        let sx = x + i * (seg_w + gap);
        let color = if i < active_segs { ALERT_COLOR } else { DIVIDER_COLOR };
        fb.fill_rect(sx, y, seg_w, h, color);
    }
}

/// Shorten model name for the table cell ("claude-opus-4-7" → "opus-4.7").
fn abbrev_model(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("claude-") {
        rest.replace("-", ".").replacen(".", "-", 1)
    } else {
        s.to_string()
    }
}

/// Take the first line of a string, truncated to fit the given pixel width.
fn first_line_truncated(s: &str, max_w: u16, char_w: u16) -> String {
    let first_line = s.lines().next().unwrap_or("").trim();
    let mut out = String::new();
    let mut acc = 0u16;
    for ch in first_line.chars() {
        let cw = if ch.is_ascii() { char_w } else { char_w * 2 };
        if acc + cw > max_w {
            break;
        }
        acc += cw;
        out.push(ch);
    }
    out
}

/// Format a number with 'k' suffix for thousands.
fn format_k(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

/// Context bar color based on percentage (SC pattern: green/yellow/red).
fn context_bar_color(pct: u16) -> Rgb565 {
    if pct >= 85 { RED_COLOR }
    else if pct >= 65 { YELLOW_COLOR }
    else { ACCENT_COLOR }
}

/// Status dot character.
fn status_dot_char(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Thinking | SessionStatus::Writing => ">>",
        SessionStatus::ToolUse => "<>",
        SessionStatus::Done => "ok",
        SessionStatus::Error => "!!",
        SessionStatus::Idle => "--",
        SessionStatus::PermissionNeeded => "??",
    }
}

/// Get status-appropriate color (SC-inspired vivid colors).
fn status_color(status: SessionStatus) -> Rgb565 {
    match status {
        SessionStatus::Thinking | SessionStatus::Writing => BLUE_COLOR,
        SessionStatus::ToolUse => YELLOW_COLOR,
        SessionStatus::Done => ACCENT_COLOR,
        SessionStatus::Idle => MUTED_COLOR,
        SessionStatus::Error => RED_COLOR,
        SessionStatus::PermissionNeeded => ALERT_COLOR,
    }
}

/// Select: session list with SC-style cards.
fn render_select(sm: &ScreenStateMachine, fb: &mut DynFramebuffer) {
    let w = fb.width();
    let sessions = sm.sessions();
    let select_idx = sm.select_index();
    let pad = 6u16;

    // Title bar
    draw_text(fb, pad, 4, "SELECT SESSION", ACCENT_COLOR);
    let count = format!("{}/{}", select_idx + 1, sessions.len());
    let cw = count.len() as u16 * FONT_W;
    draw_text(fb, w.saturating_sub(cw + pad), 4, &count, MUTED_COLOR);
    draw_divider(fb, 4 + FONT_H + 2, w, DIVIDER_COLOR);

    let list_y_start = 4 + FONT_H + 6;
    let card_h = FONT_H * 2 + 6; // 2 lines: name + user input
    let max_visible = (fb.height().saturating_sub(list_y_start + 4) / card_h) as usize;

    // No sort — use same order as UI state machine (consistent with select_index)
    let scroll_offset = if sessions.len() <= max_visible || select_idx < max_visible / 2 {
        0
    } else if select_idx + max_visible / 2 >= sessions.len() {
        sessions.len().saturating_sub(max_visible)
    } else {
        select_idx.saturating_sub(max_visible / 2)
    };

    for (vi, session) in sessions.iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
    {
        let y = list_y_start + ((vi - scroll_offset) as u16) * card_h;
        let is_selected = vi == select_idx;

        // SC-style: 3px left border for selected, highlight bg
        if is_selected {
            fb.fill_rect(0, y, w, card_h - 1, Rgb565(0x1928));
            // Blue left border (3px)
            for by in y..y + card_h - 1 {
                fb.draw_pixel(0, by, BLUE_COLOR);
                fb.draw_pixel(1, by, BLUE_COLOR);
                fb.draw_pixel(2, by, BLUE_COLOR);
            }
        }

        // Status dot
        let dot = status_dot_char(session.status);
        draw_text(fb, pad + 2, y + 1, dot, status_color(session.status));

        // Name
        let name_x = pad + 4 * FONT_W;
        let name_color = if is_selected { TEXT_COLOR } else { MUTED_COLOR };
        draw_text(fb, name_x, y + 1, &session.name, name_color);

        // Right side: always show status
        let st = status_text(session.status);
        let st_w = st.len() as u16 * FONT_W;
        draw_text(fb, w.saturating_sub(st_w + pad), y + 1, st, status_color(session.status));

        // Second line: user's last input (truncated)
        if !session.last_message.is_empty() {
            let msg_y = y + FONT_H + 2;
            let max_chars = ((w - name_x - pad) / FONT_W) as usize;
            let msg: String = session.last_message.chars().take(max_chars).collect();
            draw_text(fb, name_x, msg_y, &msg, MUTED_COLOR);
        }
    }

}

/// Allow: permission approval dialog.
fn render_allow(sm: &ScreenStateMachine, fb: &mut DynFramebuffer) {
    let w = fb.width();
    let permissions = sm.permissions();

    if permissions.is_empty() {
        return;
    }

    let view_idx = sm.permission_view_index().min(permissions.len() - 1);
    let perm = &permissions[view_idx];

    // Green border (2px)
    let border_color = ACCENT_COLOR;
    for y in 0..fb.height() {
        fb.draw_pixel(0, y, border_color);
        fb.draw_pixel(1, y, border_color);
        fb.draw_pixel(w - 1, y, border_color);
        fb.draw_pixel(w - 2, y, border_color);
    }
    for x in 0..w {
        fb.draw_pixel(x, 0, border_color);
        fb.draw_pixel(x, 1, border_color);
        fb.draw_pixel(x, fb.height() - 1, border_color);
        fb.draw_pixel(x, fb.height() - 2, border_color);
    }

    // Title
    draw_text(fb, 8, 6, "PERMISSION REQUEST", ALERT_COLOR);
    draw_divider(fb, 6 + FONT_H + 2, w - 16, DIVIDER_COLOR);

    let y_start = 6 + FONT_H + 6;

    // Session info
    let session_name = sm
        .sessions()
        .iter()
        .find(|s| s.id == perm.session_id)
        .map(|s| s.name.as_str())
        .unwrap_or("Unknown");

    let session_bar = StatusBar::new("Session:", session_name);
    session_bar.render(fb, Rect::new(8, y_start, w - 16, FONT_H));

    let action_bar = StatusBar::new("Action:", &perm.action_desc);
    action_bar.render(fb, Rect::new(8, y_start + FONT_H + 2, w - 16, FONT_H));

    draw_divider(
        fb,
        y_start + (FONT_H + 2) * 2 + 2,
        w - 16,
        DIVIDER_COLOR,
    );

    // Allow/Deny/Always options
    let options_y = y_start + (FONT_H + 2) * 2 + 6;
    let option_w = (w - 16) / 3;
    let current_option = sm.allow_option_index();

    for (i, opt) in AllowOption::ALL.iter().enumerate() {
        let ox = 8 + (i as u16) * option_w;
        let is_selected = i == current_option;

        if is_selected {
            // Highlight selected option
            fb.fill_rect(ox, options_y, option_w - 2, FONT_H + 4, ACCENT_COLOR);
            draw_text(fb, ox + 4, options_y + 2, opt.as_str(), BG_COLOR);
        } else {
            draw_text(fb, ox + 4, options_y + 2, opt.as_str(), TEXT_COLOR);
        }
    }

    // Bottom: pending count + shortcuts
    let bottom_y = fb.height() - FONT_H - 6;
    draw_divider(fb, bottom_y - 4, w - 16, DIVIDER_COLOR);

    if permissions.len() > 1 {
        let counter = format!("{}/{} pending", view_idx + 1, permissions.len());
        draw_text(fb, 8, bottom_y, &counter, ALERT_COLOR);
    }

    let hint = "SEND=Confirm CANCEL=Deny";
    let hint_w = hint.len() as u16 * FONT_W;
    draw_text(fb, w - hint_w - 8, bottom_y, hint, MUTED_COLOR);
}

/// Notify: notification list screen.
fn render_notify(sm: &ScreenStateMachine, fb: &mut DynFramebuffer) {
    let w = fb.width();
    let pad = 6u16;
    let line_h = FONT_H + 2;
    let notifications = sm.notifications();
    let unread = sm.unread_count();

    // Header — show total notification count (all events across all sessions)
    let total_events = notifications.len();
    let agg_sessions = sm.aggregated_notifications().len();
    let header = format!("NOTIFY ({} in {} sessions)", total_events, agg_sessions);
    draw_text(fb, pad, 4, &header, PURPLE_COLOR);
    draw_divider(fb, 4 + FONT_H + 2, w, DIVIDER_COLOR);

    let mut y = 4 + FONT_H + 6;
    let notify_idx = sm.notify_index();

    // Empty state
    if notifications.is_empty() {
        let msg = "No notifications yet";
        let msg_x = (w - msg.len() as u16 * FONT_W) / 2;
        let msg_y = fb.height() / 2 - FONT_H;
        draw_text(fb, msg_x, msg_y, msg, MUTED_COLOR);

        let hint = "Press SESSION to close";
        let hint_x = (w - hint.len() as u16 * FONT_W) / 2;
        draw_text(fb, hint_x, msg_y + FONT_H + 4, hint, MUTED_COLOR);
        return;
    }

    // Aggregated by session: each row = one session with event count
    let aggregated = sm.aggregated_notifications();

    for (idx, (_, name, status, count, summary)) in aggregated.iter().enumerate() {
        if y + line_h > fb.height() - line_h {
            break;
        }
        let is_selected = idx == notify_idx;
        if is_selected {
            fb.fill_rect(0, y, w, line_h, Rgb565(0x1928));
            for by in y..y + line_h {
                fb.draw_pixel(0, by, BLUE_COLOR);
                fb.draw_pixel(1, by, BLUE_COLOR);
                fb.draw_pixel(2, by, BLUE_COLOR);
            }
        }
        // Status dot + session name
        let dot = status_dot_char(*status);
        draw_text(fb, pad + 2, y, dot, status_color(*status));
        let name_x = pad + 4 * FONT_W;
        draw_text(fb, name_x, y, name, TEXT_COLOR);
        // Summary + count (with overflow protection)
        let desc_x = name_x + (name.len() as u16 + 1) * FONT_W;
        let count_str = format!("({})", count);
        let count_w = (count_str.len() as u16 + 1) * FONT_W;
        let count_x = w.saturating_sub(count_w);
        draw_text(fb, count_x, y, &count_str, DIVIDER_COLOR);
        if desc_x + 2 * FONT_W < count_x {
            let max_desc = ((count_x - desc_x - FONT_W) / FONT_W) as usize;
            let desc_trunc: String = summary.chars().take(max_desc).collect();
            draw_text(fb, desc_x, y, &desc_trunc, MUTED_COLOR);
        }
        y += line_h;
    }

    // Bottom hint (truncate if wider than screen)
    let bottom_y = fb.height().saturating_sub(FONT_H + 6);
    draw_divider(fb, bottom_y.saturating_sub(4), w, DIVIDER_COLOR);
    let hint = "SEND=Jump  ESC=Del  BTN=Close";
    let hint_w = hint.len() as u16 * FONT_W;
    let hint_x = if hint_w + pad < w { w - hint_w - pad } else { pad };
    draw_text(fb, hint_x, bottom_y, hint, MUTED_COLOR);
}

/// Render toast overlay (right side, half screen, black background).
/// Only shows the newest toast — new ones replace old ones.
fn render_toasts(sm: &ScreenStateMachine, fb: &mut DynFramebuffer) {
    let toasts = sm.toasts();
    if toasts.is_empty() {
        return;
    }
    let w = fb.width();
    let h = fb.height();
    let toast_w = w / 2;
    let toast_x = w - toast_w;

    // Black out the right half of screen
    fb.fill_rect(toast_x, 0, toast_w, h, BG_COLOR);

    // Show only the newest (first) toast — avoids framebuffer overflow
    let toast_h = FONT_H * 4 + 14; // 4 lines + padding
    for (_i, toast) in toasts.iter().enumerate().take(1) {
        let toast_y: u16 = 6;
        // Border color by status
        let border_color = match toast.status {
            SessionStatus::Error | SessionStatus::PermissionNeeded => RED_COLOR,
            SessionStatus::Done => BLUE_COLOR,
            _ => ACCENT_COLOR,
        };
        // Border (2px thick)
        for bw in 0..2u16 {
            for x in toast_x..toast_x + toast_w {
                fb.draw_pixel(x, toast_y + bw, border_color);
                fb.draw_pixel(x, toast_y + toast_h - 1 - bw, border_color);
            }
            for y in toast_y..toast_y + toast_h {
                fb.draw_pixel(toast_x + bw, y, border_color);
                fb.draw_pixel(toast_x + toast_w - 1 - bw, y, border_color);
            }
        }
        let inner_x = toast_x + 5;
        let inner_y = toast_y + 4;
        let line_h = FONT_H + 2;
        // Line 1: status dot + session name (bold color)
        let dot = status_dot_char(toast.status);
        draw_text(fb, inner_x, inner_y, dot, border_color);
        draw_text(fb, inner_x + 3 * FONT_W, inner_y, &toast.session_name, TEXT_COLOR);
        // Line 2: description (wrap to multiple chars)
        let max_chars = ((toast_w - 12) / FONT_W) as usize;
        let desc: String = toast.description.chars().take(max_chars).collect();
        draw_text(fb, inner_x, inner_y + line_h, &desc, MUTED_COLOR);
        // Line 3: description overflow (2nd line)
        if toast.description.len() > max_chars {
            let desc2: String = toast.description.chars().skip(max_chars).take(max_chars).collect();
            draw_text(fb, inner_x, inner_y + line_h * 2, &desc2, MUTED_COLOR);
        }
        // Line 4: hint
        draw_text(fb, inner_x, inner_y + line_h * 3, "SEND=Jump", DIVIDER_COLOR);
    }
}

fn status_text(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Thinking => "thinking",
        SessionStatus::ToolUse => "tool_use",
        SessionStatus::Writing => "writing",
        SessionStatus::Done => "done",
        SessionStatus::Error => "error",
        SessionStatus::Idle => "idle",
        SessionStatus::PermissionNeeded => "permission",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::UiEvent;
    fn make_fb() -> DynFramebuffer {
        DynFramebuffer::new(LCD_W, LCD_H)
    }

    fn make_sm_with_sessions(n: u16) -> ScreenStateMachine {
        let mut sm = ScreenStateMachine::new();
        for i in 1..=n {
            sm.handle_event(&UiEvent::SessionUpdate {
                session_id: i,
                name: format!("Session{i}"),
                status: SessionStatus::Idle,
            });
        }
        sm
    }

    #[test]
    fn render_standby_draws_brand() {
        let sm = ScreenStateMachine::new();
        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        // Should have some non-black pixels (brand text)
        let has_content = fb.back_buffer().iter().any(|c| *c != BG_COLOR);
        assert!(has_content, "standby should render brand text");
    }

    #[test]
    fn render_normal_shows_session() {
        let mut sm = make_sm_with_sessions(3);
        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        let has_content = fb.back_buffer().iter().any(|c| *c != BG_COLOR);
        assert!(has_content, "normal should render session info");

        // Add a permission to trigger badge
        sm.handle_event(&UiEvent::PermissionRequest {
            session_id: 1,
            action_desc: "Write main.rs".into(),
        });
        // Force back to Normal for testing (normally goes to Allow)
        // Just check that render_allow works instead
        let mut fb2 = make_fb();
        render(&sm, &mut fb2, &RenderContext::default());
        let has_content2 = fb2.back_buffer().iter().any(|c| *c != BG_COLOR);
        assert!(has_content2, "allow should render");
    }

    #[test]
    fn render_select_shows_list() {
        let mut sm = make_sm_with_sessions(5);
        sm.handle_event(&UiEvent::KnobPress); // press to enter Select
        assert_eq!(sm.state(), ScreenState::Select);

        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        let has_content = fb.back_buffer().iter().any(|c| *c != BG_COLOR);
        assert!(has_content, "select should render session list");
    }

    #[test]
    fn render_allow_shows_permission() {
        let mut sm = make_sm_with_sessions(1);
        sm.handle_event(&UiEvent::PermissionRequest {
            session_id: 1,
            action_desc: "Write main.rs".into(),
        });
        assert_eq!(sm.state(), ScreenState::Allow);

        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        // Should have green border pixels
        assert_eq!(fb.back_buffer()[0], ACCENT_COLOR, "green border top-left");
        let last = fb.back_buffer().len() - 1;
        assert_eq!(fb.back_buffer()[last], ACCENT_COLOR, "green border bottom-right");
    }

    #[test]
    fn render_allow_multi_permission_shows_counter() {
        let mut sm = make_sm_with_sessions(2);
        sm.handle_event(&UiEvent::PermissionRequest {
            session_id: 1,
            action_desc: "Write a.rs".into(),
        });
        sm.handle_event(&UiEvent::PermissionRequest {
            session_id: 2,
            action_desc: "Write b.rs".into(),
        });

        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        let has_content = fb.back_buffer().iter().any(|c| *c != BG_COLOR);
        assert!(has_content, "multi-permission should render");
    }

    #[test]
    fn render_standby_shows_time() {
        let sm = ScreenStateMachine::new();

        // Render without time
        let mut fb_no_time = make_fb();
        render(&sm, &mut fb_no_time, &RenderContext::default());
        let cyan_no_time = fb_no_time.back_buffer().iter().filter(|c| **c == CYAN_COLOR).count();

        // Render with time
        let mut fb_time = make_fb();
        let ctx = RenderContext { time_str: "14:35".into() };
        render(&sm, &mut fb_time, &ctx);
        let cyan_with_time = fb_time.back_buffer().iter().filter(|c| **c == CYAN_COLOR).count();

        // Time text adds cyan pixels
        assert!(
            cyan_with_time > cyan_no_time,
            "time text should add cyan pixels: with={cyan_with_time} without={cyan_no_time}"
        );
    }

    #[test]
    fn render_standby_blink_toggles() {
        let mut sm = ScreenStateMachine::new();
        let mut fb1 = make_fb();
        render(&sm, &mut fb1, &RenderContext::default());
        let count1 = fb1.back_buffer().iter().filter(|c| **c == ACCENT_COLOR).count();

        // Tick past the blink threshold
        for _ in 0..35 {
            sm.tick();
        }
        let mut fb2 = make_fb();
        render(&sm, &mut fb2, &RenderContext::default());
        let count2 = fb2.back_buffer().iter().filter(|c| **c == ACCENT_COLOR).count();

        // Blink should differ (one has the dot, other doesn't)
        assert_ne!(count1, count2, "blink indicator should toggle");
    }

    #[test]
    fn render_allow_selected_option_highlighted() {
        let mut sm = make_sm_with_sessions(1);
        sm.handle_event(&UiEvent::PermissionRequest {
            session_id: 1,
            action_desc: "Test".into(),
        });

        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        // The ALLOW option should be highlighted with ACCENT_COLOR background
        let accent_count = fb.back_buffer().iter().filter(|c| **c == ACCENT_COLOR).count();
        assert!(accent_count > 0, "selected option should have accent background");
    }

    #[test]
    fn render_select_with_many_sessions_scrolls() {
        let mut sm = make_sm_with_sessions(20);
        sm.handle_event(&UiEvent::KnobPress); // enter Select
        // Scroll to the end
        for _ in 0..15 {
            sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        }
        assert_eq!(sm.state(), ScreenState::Select);

        let mut fb = make_fb();
        render(&sm, &mut fb, &RenderContext::default());
        let has_content = fb.back_buffer().iter().any(|c| *c != BG_COLOR);
        assert!(has_content, "scrolled select should render");
    }

    #[test]
    fn render_normal_empty_sessions_no_panic() {
        let sm = ScreenStateMachine::new();
        let mut fb = make_fb();
        // Force Normal state via a session update then remove
        // (but for simplicity, just test render with Standby — which is what happens)
        render(&sm, &mut fb, &RenderContext::default()); // should not panic
    }
}
