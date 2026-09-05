use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn monitor_picker_updates_tokens_and_completion_without_reopening() -> Result<()> {
    let (mut app, mut events, _ops) = Box::pin(make_test_app_with_channels()).await;
    let main = ThreadId::from_string("00000000-0000-0000-0000-000000000100")?;
    let worker = ThreadId::from_string("00000000-0000-0000-0000-000000000101")?;
    app.primary_thread_id = Some(main);
    app.active_thread_id = Some(main);
    app.agent_navigation.upsert(main, None, None, false);
    app.agent_navigation
        .upsert(worker, Some("Worker".to_string()), None, false);
    app.agent_navigation
        .monitor
        .observe(worker, &turn_started_notification(worker, "one"));
    let params = app.agent_picker_selection_view_params(Some(1));
    app.chat_widget.show_selection_view(params);
    let selected = app.agent_picker_selected_thread();
    app.agent_navigation
        .monitor
        .observe(worker, &token_usage_notification(worker, "one", None));
    app.repaint_agent_picker(selected);
    let live = render_bottom_popup(&app.chat_widget, 100);
    assert!(live.contains("Running"));
    assert!(live.contains("10 tokens"));
    insta::assert_snapshot!("live_agent_monitor", live);
    assert_eq!(app.agent_picker_selected_thread(), Some(worker));
    app.agent_navigation.monitor.observe(
        worker,
        &turn_completed_notification(worker, "one", TurnStatus::Completed),
    );
    app.repaint_agent_picker(selected);
    let folded = render_bottom_popup(&app.chat_widget, 100);
    assert!(folded.contains("Show completed (1)"));
    assert!(!folded.contains("Worker"));
    insta::assert_snapshot!("completed_agent_monitor", folded);
    app.agent_navigation.show_completed = true;
    app.repaint_agent_picker(None);
    let expanded = render_bottom_popup(&app.chat_widget, 100);
    assert!(expanded.contains("Completed"));
    assert!(expanded.contains("10 tokens"));
    insta::assert_snapshot!("expanded_agent_monitor", expanded);
    // Selecting a completed row still enters the existing transcript path.
    let params = app.agent_picker_selection_view_params(Some(1));
    app.chat_widget.replace_selection_view_if_present(
        super::super::agent_picker::AGENT_PICKER_VIEW_ID,
        params,
    );
    while events.try_recv().is_ok() {}
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(events.try_recv(), Ok(AppEvent::SelectAgentThread(id)) if id == worker);
    Ok(())
}

#[tokio::test]
async fn monitor_picker_keeps_failed_agents_visible_and_fits_narrow_terminal() -> Result<()> {
    let mut app = Box::pin(make_test_app()).await;
    let worker = ThreadId::from_string("00000000-0000-0000-0000-000000000101")?;
    app.agent_navigation
        .upsert(worker, Some("Worker".to_string()), None, false);
    app.agent_navigation
        .monitor
        .observe(worker, &turn_started_notification(worker, "one"));
    app.agent_navigation.monitor.observe(
        worker,
        &turn_completed_notification(worker, "one", TurnStatus::Failed),
    );
    let params = app.agent_picker_selection_view_params(None);
    app.chat_widget.show_selection_view(params);
    let narrow = render_bottom_popup(&app.chat_widget, 45);
    assert!(narrow.contains("Failed"));
    assert!(narrow.contains("Worker"));
    insta::assert_snapshot!("narrow_agent_monitor", narrow);
    Ok(())
}
