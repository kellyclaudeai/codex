//! Focus routing for the persistent active-agent list, before global Escape handling.
use super::agent_panel::AgentPanelAction;
use super::agent_panel::AgentPanelRow;
use super::*;
use crossterm::event::KeyModifiers;

impl BottomPane {
    pub(crate) fn set_active_agent_panel(
        &mut self,
        rows: Vec<AgentPanelRow>,
        return_thread: Option<ThreadId>,
    ) {
        self.agent_panel_return_thread = return_thread;
        if self.agent_panel.set_rows(rows) {
            self.request_redraw();
        }
    }

    pub(crate) fn handle_agent_panel_key(&mut self, key: KeyEvent) -> bool {
        if !self.view_stack.is_empty()
            || self.composer.popup_active()
            || self.questions.as_ref().is_some_and(|q| q.expanded)
            || self.composer.is_in_paste_burst()
            || key.kind == KeyEventKind::Release
        {
            return false;
        }
        if !self.agent_panel.is_focused() {
            if !self.composer.is_empty() || key.modifiers != KeyModifiers::NONE {
                return false;
            }
            if key.code == KeyCode::Esc
                && let Some(thread) = self.agent_panel_return_thread
            {
                self.app_event_tx.send(AppEvent::SelectAgentThread(thread));
                return true;
            }
            if key.code != KeyCode::Down || self.agent_panel.is_empty() {
                return false;
            }
            self.agent_panel.focus();
        } else {
            match self.agent_panel.handle_key_event(key) {
                AgentPanelAction::Select(thread) => {
                    self.agent_panel.unfocus();
                    self.app_event_tx.send(AppEvent::SelectAgentThread(thread));
                }
                AgentPanelAction::ReturnToPrompt | AgentPanelAction::Consumed => {}
                AgentPanelAction::Ignored => {
                    self.agent_panel.unfocus();
                    self.request_redraw();
                    return false;
                }
            }
        }
        self.request_redraw();
        true
    }
}
