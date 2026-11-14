//! Control Tower overlay (default Alt+W): a read-only overview of the
//! session's own runtime state — swarm fleet, background tasks, and recent
//! activity — built from the same `App` fields the normal chat view already
//! reads. It is another *view* of the running session, not a second runtime:
//! opening it mutates no agent state, and closing it returns to the chat
//! exactly where the user left it (same scroll offset, same draft input).

use super::*;
use crate::tui::TuiState;

/// State of the open Control Tower overlay. Holds no runtime data itself —
/// everything it shows is read fresh from `App` each frame — only the local
/// scroll/selection the user is navigating with.
#[derive(Debug, Clone, Default)]
pub struct ControlTowerOverlay {
    pub scroll: usize,
}

impl App {
    /// Open the Control Tower overlay, or close it if already open. Does not
    /// touch `session`, `display_messages`, `scroll_offset`, or `input`: those
    /// belong to the chat view and are left exactly as they were.
    pub(super) fn toggle_control_tower(&mut self) {
        if self.control_tower_overlay.is_some() {
            self.control_tower_overlay = None;
            return;
        }
        self.control_tower_overlay = Some(ControlTowerOverlay::default());
    }

    pub(super) fn handle_control_tower_key(&mut self, code: KeyCode) -> Result<()> {
        if self.control_tower_overlay.is_none() {
            return Ok(());
        }
        let max = self.control_tower_rows_ref().len().saturating_sub(1);
        let overlay = self.control_tower_overlay.as_mut().expect("checked above");
        let scroll = overlay.scroll;
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.control_tower_overlay = None;
                return Ok(());
            }
            KeyCode::Down | KeyCode::Char('j') => overlay.scroll = scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => overlay.scroll = scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') | KeyCode::Char('d') => {
                overlay.scroll = scroll.saturating_add(20)
            }
            KeyCode::PageUp | KeyCode::Char('u') => overlay.scroll = scroll.saturating_sub(20),
            KeyCode::Home | KeyCode::Char('g') => overlay.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => overlay.scroll = max,
            _ => {}
        }
        if let Some(overlay) = self.control_tower_overlay.as_mut() {
            overlay.scroll = overlay.scroll.min(max);
        }
        Ok(())
    }

    /// Live activity rows for the Control Tower feed: real swarm-member and
    /// background-task state, not fabricated progress. Ordering is stable
    /// (swarm members first, then background tasks) so the scroll position
    /// does not jump while the overlay is open.
    pub(crate) fn control_tower_rows_ref(&self) -> Vec<ControlTowerRow> {
        let mut rows = Vec::new();
        for member in self.swarm_members_for_transcript() {
            rows.push(ControlTowerRow::SwarmMember(member));
        }
        for task in self.background_task_rows_ref() {
            rows.push(ControlTowerRow::BackgroundTask(task.clone()));
        }
        rows
    }
}

/// One row in the Control Tower activity feed.
#[derive(Debug, Clone)]
pub enum ControlTowerRow {
    SwarmMember(crate::protocol::SwarmMemberStatus),
    BackgroundTask(crate::tui::BackgroundTaskRow),
}

#[cfg(test)]
mod tests {
    use crate::provider::Provider;
    use std::sync::Arc;

    struct MockProvider;

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        async fn complete(
            &self,
            _messages: &[crate::message::Message],
            _tools: &[crate::message::ToolDefinition],
            _system: &str,
            _resume_session_id: Option<&str>,
        ) -> anyhow::Result<crate::provider::EventStream> {
            Err(anyhow::anyhow!("mock provider does not stream in this test"))
        }
        fn name(&self) -> &str {
            "mock"
        }
        fn fork(&self) -> Arc<dyn Provider> {
            Arc::new(Self)
        }
    }

    /// Alt+W, dispatched through the real key-handling path (not the toggle
    /// fn directly), must open the overlay and Esc must close it again,
    /// without touching chat state: same draft input, same scroll offset,
    /// same session. It is a second *view*, not a second runtime or a reset
    /// of the current one. (Esc/q close, matching every other overlay
    /// sharing `handle_scroll_overlay_key`: once an overlay is open it owns
    /// all keys, so re-pressing the open chord does not reach the toggle fn
    /// — same as `commit_log_overlay`.)
    #[test]
    fn alt_w_opens_and_esc_closes_control_tower_through_real_dispatch() {
        let provider: Arc<dyn Provider> = Arc::new(MockProvider);
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
        let mut app = crate::tui::app::App::new_for_test_harness(provider, registry);
        app.input = "unsent draft".to_string();
        app.scroll_offset = 7;

        use crossterm::event::{KeyCode, KeyModifiers};

        assert!(app.control_tower_overlay.is_none());
        app.handle_key(KeyCode::Char('w'), KeyModifiers::ALT)
            .unwrap();
        assert!(
            app.control_tower_overlay.is_some(),
            "Alt+W should open the Control Tower overlay"
        );
        assert_eq!(app.input, "unsent draft");
        assert_eq!(app.scroll_offset, 7);

        app.handle_key(KeyCode::Esc, KeyModifiers::empty()).unwrap();
        assert!(app.control_tower_overlay.is_none());
        assert_eq!(app.input, "unsent draft");
        assert_eq!(app.scroll_offset, 7);
    }
}
