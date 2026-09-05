//! Compact live state for rows in the `/subagents` picker.
//!
//! This state is retained separately from bounded transcript event buffers. Token totals are
//! authoritative cumulative snapshots, and activity text is deliberately limited to safe item
//! classes rather than command text or tool arguments.

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

const MAX_LABEL_CHARS: usize = 96;
const MAX_MONITORED_THREADS: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentMonitorStatus {
    Running,
    Waiting,
    Idle,
    Completed,
    Interrupted,
    Failed,
    SystemError,
    Closed,
    Unknown,
}

impl AgentMonitorStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Waiting => "Waiting",
            Self::Idle => "Idle",
            Self::Completed => "Completed",
            Self::Interrupted => "Interrupted",
            Self::Failed => "Failed",
            Self::SystemError => "Error",
            Self::Closed => "Closed",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentTurnResult {
    Completed,
    Interrupted,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentMonitorDescription {
    pub(super) status: AgentMonitorStatus,
    pub(super) latest_turn_result: Option<AgentTurnResult>,
    pub(super) total_tokens: Option<i64>,
    pub(super) activity: Option<String>,
    pub(super) model: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AgentMonitorEntry {
    status: Option<AgentMonitorStatus>,
    latest_turn_result: Option<AgentTurnResult>,
    total_tokens: Option<i64>,
    activity: Option<String>,
    model: Option<String>,
    active_turn: Option<u64>,
    active_item: Option<u64>,
    observed_live: bool,
}

#[derive(Debug, Default)]
pub(super) struct AgentMonitorState {
    entries: HashMap<ThreadId, AgentMonitorEntry>,
}

impl AgentMonitorState {
    pub(super) fn seed(&mut self, thread_id: ThreadId, thread: &Thread) {
        if self.entries.len() >= MAX_MONITORED_THREADS && !self.entries.contains_key(&thread_id) {
            return;
        }
        let entry = self.entries.entry(thread_id).or_default();
        if !entry.observed_live {
            entry.status = Some(status_from_thread(&thread.status));
            if matches!(
                thread.status,
                ThreadStatus::Active { .. } | ThreadStatus::SystemError
            ) {
                entry.latest_turn_result = None;
            } else if let Some(result) = thread
                .turns
                .last()
                .and_then(|turn| turn_result(&turn.status))
            {
                entry.latest_turn_result = Some(result);
                entry.status = Some(status_from_result(result));
            }
        }
        if entry.model.is_none()
            && let Some(model) = thread.model.as_deref()
        {
            entry.model = bounded_label(model);
        }
    }

    /// Observes one already-routed notification and reports whether display state changed.
    pub(super) fn observe(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) -> bool {
        if self.entries.len() >= MAX_MONITORED_THREADS && !self.entries.contains_key(&thread_id) {
            return false;
        }
        let before = self.entries.get(&thread_id).cloned().unwrap_or_default();
        let entry = self.entries.entry(thread_id).or_default();
        match notification {
            ServerNotification::ThreadStarted(started) => {
                entry.observed_live = true;
                entry.status = Some(status_from_thread(&started.thread.status));
                if let Some(model) = started.thread.model.as_deref() {
                    entry.model = bounded_label(model);
                }
            }
            ServerNotification::ThreadStatusChanged(changed) => {
                entry.observed_live = true;
                entry.status = Some(status_from_thread(&changed.status));
                if matches!(
                    &changed.status,
                    ThreadStatus::Active { .. } | ThreadStatus::SystemError
                ) {
                    entry.latest_turn_result = None;
                }
            }
            ServerNotification::ThreadClosed(_)
            | ServerNotification::ThreadArchived(_)
            | ServerNotification::ThreadDeleted(_) => {
                entry.observed_live = true;
                entry.status = Some(AgentMonitorStatus::Closed);
                entry.activity = None;
                entry.active_item = None;
            }
            ServerNotification::ThreadSettingsUpdated(settings) => {
                entry.observed_live = true;
                entry.model = bounded_label(&settings.thread_settings.model);
            }
            ServerNotification::ModelRerouted(rerouted) => {
                entry.observed_live = true;
                entry.model = bounded_label(&rerouted.to_model);
            }
            ServerNotification::ModelSafetyBufferingUpdated(buffering) => {
                entry.observed_live = true;
                entry.model = bounded_label(&buffering.model);
            }
            ServerNotification::TurnStarted(started) => {
                entry.observed_live = true;
                entry.status = Some(AgentMonitorStatus::Running);
                entry.latest_turn_result = None;
                entry.activity = None;
                entry.active_turn = Some(bounded_id(&started.turn.id));
                entry.active_item = None;
            }
            ServerNotification::TurnCompleted(completed) => {
                let turn_id = bounded_id(&completed.turn.id);
                if entry
                    .active_turn
                    .is_some_and(|active_turn| active_turn != turn_id)
                {
                    return false;
                }
                entry.observed_live = true;
                entry.latest_turn_result = turn_result(&completed.turn.status);
                entry.status = entry.latest_turn_result.map(status_from_result);
                entry.active_turn = None;
                entry.active_item = None;
                entry.activity = None;
            }
            ServerNotification::ThreadTokenUsageUpdated(usage) => {
                entry.observed_live = true;
                // This is a cumulative thread snapshot. Replacing it avoids double counting.
                entry.total_tokens = Some(usage.token_usage.total.total_tokens);
            }
            ServerNotification::ItemStarted(started) => {
                entry.observed_live = true;
                entry.activity = Some(activity_label(&started.item));
                entry.active_item = Some(bounded_id(started.item.id()));
            }
            ServerNotification::ItemCompleted(completed) => {
                entry.observed_live = true;
                if entry
                    .active_item
                    .is_some_and(|active_item| active_item == bounded_id(completed.item.id()))
                {
                    entry.activity = None;
                    entry.active_item = None;
                }
            }
            _ => {}
        }
        before != *entry
    }

    pub(super) fn describe(
        &self,
        thread_id: ThreadId,
        fallback_running: bool,
        fallback_closed: bool,
    ) -> AgentMonitorDescription {
        let entry = self.entries.get(&thread_id);
        let status = if let Some(result) = entry.and_then(|entry| entry.latest_turn_result) {
            status_from_result(result)
        } else if let Some(status) = entry.and_then(|entry| entry.status) {
            status
        } else if fallback_closed {
            AgentMonitorStatus::Closed
        } else if fallback_running {
            AgentMonitorStatus::Running
        } else {
            AgentMonitorStatus::Unknown
        };
        AgentMonitorDescription {
            status,
            latest_turn_result: entry.and_then(|entry| entry.latest_turn_result),
            total_tokens: entry.and_then(|entry| entry.total_tokens),
            activity: entry.and_then(|entry| entry.activity.clone()),
            model: entry.and_then(|entry| entry.model.clone()),
        }
    }

    pub(super) fn is_successfully_completed(&self, thread_id: ThreadId) -> bool {
        self.entries.get(&thread_id).is_some_and(|entry| {
            entry.latest_turn_result == Some(AgentTurnResult::Completed)
                || entry.status == Some(AgentMonitorStatus::Completed)
        })
    }

    pub(super) fn remove(&mut self, thread_id: ThreadId) {
        self.entries.remove(&thread_id);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

fn status_from_thread(status: &ThreadStatus) -> AgentMonitorStatus {
    match status {
        ThreadStatus::Active { active_flags }
            if active_flags.contains(&ThreadActiveFlag::WaitingOnApproval)
                || active_flags.contains(&ThreadActiveFlag::WaitingOnUserInput) =>
        {
            AgentMonitorStatus::Waiting
        }
        ThreadStatus::Active { .. } => AgentMonitorStatus::Running,
        ThreadStatus::Idle => AgentMonitorStatus::Idle,
        ThreadStatus::SystemError => AgentMonitorStatus::SystemError,
        ThreadStatus::NotLoaded => AgentMonitorStatus::Closed,
    }
}

fn turn_result(status: &TurnStatus) -> Option<AgentTurnResult> {
    match status {
        TurnStatus::Completed => Some(AgentTurnResult::Completed),
        TurnStatus::Interrupted => Some(AgentTurnResult::Interrupted),
        TurnStatus::Failed => Some(AgentTurnResult::Failed),
        TurnStatus::InProgress => None,
    }
}

fn status_from_result(result: AgentTurnResult) -> AgentMonitorStatus {
    match result {
        AgentTurnResult::Completed => AgentMonitorStatus::Completed,
        AgentTurnResult::Interrupted => AgentMonitorStatus::Interrupted,
        AgentTurnResult::Failed => AgentMonitorStatus::Failed,
    }
}

fn activity_label(item: &ThreadItem) -> String {
    match item {
        ThreadItem::UserMessage { .. } => "Reading instructions",
        ThreadItem::HookPrompt { .. } => "Running hook",
        ThreadItem::AgentMessage { .. } => "Writing response",
        ThreadItem::FunctionCallOutput { .. } => "Processing tool result",
        ThreadItem::Plan { .. } => "Updating plan",
        ThreadItem::Reasoning { .. } => "Reasoning",
        ThreadItem::CommandExecution { .. } => "Running command",
        ThreadItem::FileChange { .. } => "Editing files",
        ThreadItem::McpToolCall { server, tool, .. } => {
            return bounded_label(&format!("Using {server}/{tool}"))
                .unwrap_or_else(|| "Using tool".to_string());
        }
        ThreadItem::DynamicToolCall {
            namespace, tool, ..
        } => {
            let name = namespace
                .as_deref()
                .map(|namespace| format!("{namespace}/{tool}"))
                .unwrap_or_else(|| tool.clone());
            return bounded_label(&format!("Using {name}"))
                .unwrap_or_else(|| "Using tool".to_string());
        }
        ThreadItem::CollabAgentToolCall { .. } => "Coordinating agents",
        ThreadItem::SubAgentActivity { .. } => "Coordinating agents",
        ThreadItem::WebSearch(_) => "Searching the web",
        ThreadItem::ImageView { .. } => "Viewing image",
        ThreadItem::Sleep(_) => "Waiting",
        ThreadItem::ImageGeneration(_) => "Generating image",
        ThreadItem::EnteredReviewMode { .. } => "Reviewing changes",
        ThreadItem::ExitedReviewMode { .. } => "Finishing review",
        ThreadItem::ContextCompaction { .. } => "Compacting context",
    }
    .to_string()
}

fn bounded_label(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        value
            .chars()
            .take(MAX_LABEL_CHARS)
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect(),
    )
}

fn bounded_id(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "agent_monitor_tests.rs"]
mod tests;
