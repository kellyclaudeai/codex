//! Keeps the persistent panel scoped to active children of this primary session.
use super::agent_monitor::AgentMonitorStatus;
use super::*;
use crate::bottom_pane::agent_panel::AgentPanelRow;

impl App {
    pub(super) fn sync_active_agent_panel(&mut self) {
        let rows = self
            .agent_navigation
            .ordered_threads()
            .into_iter()
            .filter_map(|(id, entry)| {
                if self.primary_thread_id == Some(id) || self.side_threads.contains_key(&id) {
                    return None;
                }
                let state =
                    self.agent_navigation
                        .monitor
                        .describe(id, entry.is_running, entry.is_closed);
                if !matches!(
                    state.status,
                    AgentMonitorStatus::Running | AgentMonitorStatus::Waiting
                ) {
                    return None;
                }
                let name = entry.agent_path.clone().unwrap_or_else(|| {
                    format_agent_picker_item_name(
                        entry.agent_nickname.as_deref(),
                        entry.agent_role.as_deref(),
                        /*is_primary*/ false,
                    )
                });
                let tokens = state
                    .total_tokens
                    .map(|count| format!("{count} tokens"))
                    .unwrap_or_else(|| "tokens unavailable".to_string());
                let model = state.model.as_deref().unwrap_or("model unknown");
                let mut detail = format!("{} · {tokens} · {model}", state.status.label());
                if let Some(activity) = state.activity {
                    detail.push_str(&format!(" · {activity}"));
                }
                Some(AgentPanelRow {
                    thread_id: id,
                    name,
                    detail,
                })
            })
            .collect();
        let return_thread = self
            .primary_thread_id
            .filter(|id| self.active_thread_id != Some(*id));
        self.chat_widget.set_active_agent_panel(rows, return_thread);
    }
}
