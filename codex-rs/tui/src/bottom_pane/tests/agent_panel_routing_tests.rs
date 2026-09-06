use super::*;
use crate::bottom_pane::agent_panel::AgentPanelRow;
use pretty_assertions::assert_eq;

#[test]
fn active_agent_panel_preserves_drafts_and_hides_cursor_only_when_focused() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_active_agent_panel(
        vec![AgentPanelRow {
            thread_id: ThreadId::from_u128(1),
            name: "worker".to_string(),
            detail: "Running · 10 tokens".to_string(),
        }],
        None,
    );
    pane.composer
        .set_text_content("draft\nsecond line".to_string(), Vec::new(), Vec::new());
    assert!(!pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert!(!pane.agent_panel.is_focused());
    pane.composer
        .set_text_content(String::new(), Vec::new(), Vec::new());
    assert!(!pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT)));
    assert!(pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    let area = Rect::new(0, 0, 80, pane.desired_height(80));
    assert_eq!(pane.cursor_pos(area), None);
    assert!(pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(pane.cursor_pos(area).is_some());
    assert!(pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    pane.handle_paste("pasted draft".to_string());
    assert!(!pane.agent_panel.is_focused());
    assert!(!pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
}

#[test]
fn escape_from_inspected_agent_returns_to_main_without_interrupting() {
    let (tx, mut rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    let main = ThreadId::from_u128(1);
    pane.set_active_agent_panel(Vec::new(), Some(main));
    assert!(pane.handle_agent_panel_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(matches!(rx.try_recv(), Ok(AppEvent::SelectAgentThread(id)) if id == main));
    assert!(rx.try_recv().is_err());
}
