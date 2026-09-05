use super::*;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::TokenUsageBreakdown;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnItemsView;
use pretty_assertions::assert_eq;

fn thread_id() -> ThreadId {
    ThreadId::from_string("0198a9d1-7f58-7d42-99e8-c6262c249001").unwrap()
}

fn turn(status: TurnStatus) -> Turn {
    Turn {
        id: "turn-1".to_string(),
        items: Vec::new(),
        items_view: TurnItemsView::Full,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn total_usage(total_tokens: i64) -> ThreadTokenUsage {
    let total = TokenUsageBreakdown {
        total_tokens,
        input_tokens: total_tokens,
        cached_input_tokens: 0,
        cache_write_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
    };
    ThreadTokenUsage {
        total: total.clone(),
        last: total,
        model_context_window: None,
    }
}

#[test]
fn idle_status_preserves_the_latest_completed_turn() {
    let thread_id = thread_id();
    let mut state = AgentMonitorState::default();
    state.observe(
        thread_id,
        &ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: turn(TurnStatus::Completed),
        }),
    );
    state.observe(
        thread_id,
        &ServerNotification::ThreadStatusChanged(ThreadStatusChangedNotification {
            thread_id: thread_id.to_string(),
            status: ThreadStatus::Idle,
        }),
    );

    let description = state.describe(thread_id, false, false);
    assert_eq!(description.status, AgentMonitorStatus::Completed);
    assert_eq!(
        description.latest_turn_result,
        Some(AgentTurnResult::Completed)
    );
    assert!(state.is_successfully_completed(thread_id));
}

#[test]
fn cumulative_token_snapshots_replace_instead_of_accumulate() {
    let thread_id = thread_id();
    let mut state = AgentMonitorState::default();
    for total_tokens in [120, 175] {
        state.observe(
            thread_id,
            &ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                token_usage: total_usage(total_tokens),
            }),
        );
    }

    assert_eq!(
        state.describe(thread_id, false, false).total_tokens,
        Some(175)
    );
}

#[test]
fn command_activity_does_not_include_command_text() {
    let thread_id = thread_id();
    let mut state = AgentMonitorState::default();
    let item: ThreadItem = serde_json::from_value(serde_json::json!({
        "type": "commandExecution",
        "id": "item-1",
        "pluginId": null,
        "scriptPath": null,
        "command": "print-secret --token private",
        "cwd": "/tmp",
        "processId": null,
        "source": "agent",
        "status": "inProgress",
        "commandActions": [],
        "aggregatedOutput": null,
        "exitCode": null,
        "durationMs": null
    }))
    .unwrap();
    state.observe(
        thread_id,
        &ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            started_at_ms: 0,
            item,
        }),
    );

    let description = state.describe(thread_id, true, false);
    assert_eq!(description.activity.as_deref(), Some("Running command"));
}

#[test]
fn monitor_description_snapshot() {
    let thread_id = thread_id();
    let mut state = AgentMonitorState::default();
    state.observe(
        thread_id,
        &ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: turn(TurnStatus::Failed),
        }),
    );
    state.observe(
        thread_id,
        &ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            token_usage: total_usage(12_345),
        }),
    );

    insta::assert_debug_snapshot!(state.describe(thread_id, false, false));
}

#[test]
fn resumed_agent_is_visible_and_ignores_previous_turn_completion() {
    let id = thread_id();
    let mut state = AgentMonitorState::default();
    state.observe(id, &ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: id.to_string(), turn: turn(TurnStatus::Completed),
    }));
    let mut next = turn(TurnStatus::InProgress);
    next.id = "turn-2".to_string();
    state.observe(id, &ServerNotification::TurnStarted(codex_app_server_protocol::TurnStartedNotification {
        thread_id: id.to_string(), turn: next,
    }));
    state.observe(id, &ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: id.to_string(), turn: turn(TurnStatus::Completed),
    }));
    assert_eq!(state.describe(id, false, true).status, AgentMonitorStatus::Running);
    assert!(!state.is_successfully_completed(id));
    state.observe(id, &ServerNotification::ThreadStatusChanged(ThreadStatusChangedNotification {
        thread_id: id.to_string(), status: ThreadStatus::SystemError,
    }));
    assert_eq!(state.describe(id, false, false).status, AgentMonitorStatus::SystemError);
}
