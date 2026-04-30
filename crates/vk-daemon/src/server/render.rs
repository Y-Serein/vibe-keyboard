//! Daemon-side render loop — renders LCD framebuffer for GUI Canvas.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::state::DaemonState;

/// Daemon-side render loop — renders LCD framebuffer for GUI Canvas.
pub(super) async fn run_render_loop(state: Arc<DaemonState>) {
    let w = state.lcd_width;
    let h = state.lcd_height;
    let mut fb = vk_display::framebuffer::DynFramebuffer::new(w, h);
    let ctx = vk_ui::renderer::RenderContext {
        time_str: String::new(),
    };
    // Track last-seen notification ID to detect new arrivals → trigger toast.
    // Monotonic ID is immune to queue shrinking (remove/mark_read).
    let mut last_seen_notif_id: u32 = 0;
    // T16.1: dirty-flag rendering — skip render if nothing changed and no animation.
    let mut last_rendered_gen: u64 = 0;
    let render_interval = std::time::Duration::from_millis(100); // 10fps

    loop {
        tokio::time::sleep(render_interval).await;

        // T16.1: Check if re-render is needed
        let current_gen = state.render_generation.load(Ordering::Relaxed);
        let has_animation = {
            let ui = state.ui_state.read().await;
            ui.has_active_animation()
        };
        if current_gen == last_rendered_gen && !has_animation {
            continue;
        }
        last_rendered_gen = current_gen;

        // ── Prepare data OUTSIDE the ui_state lock (T16.7: snapshot before render) ──
        let sessions = {
            let store = state.store.read().await;
            store.to_protocol_list()
        };

        let (entries, new_toasts) = {
            let nq = state.notification_queue.read().await;
            let all = nq.all();

            // Collect all notifications with id > last_seen — these are genuinely new.
            let toasts: Vec<_> = all.iter()
                .filter(|n| n.id > last_seen_notif_id)
                .map(|n| (n.session_id, n.session_name.clone(), n.description.clone(), n.status))
                .collect();
            if let Some(max_id) = all.iter().map(|n| n.id).max() {
                last_seen_notif_id = max_id;
            }

            let entries: Vec<vk_ui::screen::NotificationEntry> = all.into_iter()
                .map(|n| {
                    let proto: vk_protocol::message::NotificationInfo = n.into();
                    proto.into()
                })
                .collect();
            (entries, toasts)
        };

        // T16.7 fix: Short write lock for event/tick, then render from READ lock
        {
            let mut ui = state.ui_state.write().await;
            ui.handle_event(&vk_ui::event::UiEvent::SessionListReplace { sessions });
            for (sid, name, desc, status) in new_toasts {
                ui.show_toast(sid, name, desc, status);
            }
            ui.set_notifications(entries);
            ui.tick();
        }
        // Render outside write lock — only needs read access
        {
            let ui = state.ui_state.read().await;
            vk_ui::renderer::render(&ui, &mut fb, &ctx);
        }
        fb.swap();

        // P0-4: Bytes::copy_from_slice — one alloc + one memcpy per dirty frame.
        // True zero-alloc would need Arc<[u8]> double-buffer; this is honest and simple.
        let frame_bytes = bytes::Bytes::copy_from_slice(fb.front_buffer_as_bytes());
        let mut frame = state.frame_buffer.write().await;
        *frame = frame_bytes;
    }
}
