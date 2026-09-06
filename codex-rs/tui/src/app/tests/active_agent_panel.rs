use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn active_agent_panel_filters_finished_agents_and_routes_down_enter() -> Result<()> {
    let (mut app, mut events, _ops) = Box::pin(make_test_app_with_channels()).await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let main = ThreadId::from_u128(100);
    let worker = ThreadId::from_u128(101);
    let old = ThreadId::from_u128(102);
    app.primary_thread_id = Some(main);
    app.active_thread_id = Some(main);
    for (id, name) in [(main, "Main"), (worker, "Research"), (old, "Finished")] {
        app.agent_navigation
            .upsert(id, Some(name.to_string()), None, false);
        app.agent_navigation
            .monitor
            .observe(id, &turn_started_notification(id, "one"));
    }
    app.agent_navigation.monitor.observe(
        old,
        &turn_completed_notification(old, "one", TurnStatus::Completed),
    );
    app.agent_navigation
        .monitor
        .observe(worker, &token_usage_notification(worker, "one", None));
    app.sync_active_agent_panel();
    let rendered = render_bottom_popup(&app.chat_widget, 100);
    assert!(rendered.contains("Active agents (1)"));
    assert!(rendered.contains("Research"));
    assert!(!rendered.contains("Finished"));
    insta::assert_snapshot!("persistent_active_panel", rendered);
    while events.try_recv().is_ok() {}
    app.handle_key_event(
        &mut tui,
        &mut server,
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
    )
    .await;
    app.handle_key_event(
        &mut tui,
        &mut server,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    )
    .await;
    assert!(!app.backtrack.primed);
    assert!(
        events.try_recv().is_err(),
        "Escape returns focus without interrupting"
    );
    app.handle_key_event(
        &mut tui,
        &mut server,
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
    )
    .await;
    app.handle_key_event(
        &mut tui,
        &mut server,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )
    .await;
    assert_matches!(events.try_recv(), Ok(AppEvent::SelectAgentThread(id)) if id == worker);
    app.agent_navigation.monitor.observe(
        worker,
        &turn_completed_notification(worker, "one", TurnStatus::Completed),
    );
    app.sync_active_agent_panel();
    assert!(!render_bottom_popup(&app.chat_widget, 100).contains("Active agents"));

    app.open_agent_picker(&mut server).await;
    assert_eq!(app.agent_navigation.show_completed, true);
    let history = render_bottom_popup(&app.chat_widget, 100);
    assert!(history.contains("Research"));
    assert!(history.contains("Finished"));
    Ok(())
}
