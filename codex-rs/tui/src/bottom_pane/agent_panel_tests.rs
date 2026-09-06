use super::*;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn id(value: u128) -> ThreadId {
    ThreadId::from_u128(value)
}

fn row(value: u128, name: &str, detail: &str) -> AgentPanelRow {
    AgentPanelRow {
        thread_id: id(value),
        name: name.to_string(),
        detail: detail.to_string(),
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn snapshot(panel: &AgentPanel, width: u16, height: u16) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    panel.render(area, &mut buf);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn renders_persistent_and_focused_states() {
    let mut panel = AgentPanel::default();
    panel.set_rows(vec![
        row(1, "research", "running"),
        row(2, "tests", "waiting"),
    ]);

    insta::assert_snapshot!(snapshot(&panel, 60, panel.desired_height(60)), @r"
      Active agents (2)  ↓ browse  /subagents all
      research  running
      tests  waiting
    ");

    panel.focus();
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Down)),
        AgentPanelAction::Consumed
    );
    insta::assert_snapshot!(snapshot(&panel, 60, panel.desired_height(60)), @r"
      Active agents (2)  ↑↓ navigate  enter open  esc prompt
      research  running
    › tests  waiting
    ");
}

#[test]
fn caps_rows_and_scrolls_to_selection() {
    let mut panel = AgentPanel::default();
    panel.set_rows(
        (1..=8)
            .map(|value| row(value, &format!("agent-{value}"), "running"))
            .collect(),
    );
    panel.focus();
    for _ in 0..7 {
        assert_eq!(
            panel.handle_key_event(key(KeyCode::Down)),
            AgentPanelAction::Consumed
        );
    }

    assert_eq!(panel.desired_height(/*width*/ 60), 7);
    insta::assert_snapshot!(snapshot(&panel, 60, panel.desired_height(60)), @r"
      Active agents (8)  ↑↓ navigate  enter open  esc prompt
      agent-3  running
      agent-4  running
      agent-5  running
      agent-6  running
      agent-7  running
    › agent-8  running
    ");
}

#[test]
fn keyboard_selects_and_returns_to_prompt() {
    let mut panel = AgentPanel::default();
    panel.set_rows(vec![row(1, "one", "running"), row(2, "two", "waiting")]);

    assert_eq!(
        panel.handle_key_event(key(KeyCode::Down)),
        AgentPanelAction::Ignored
    );
    panel.focus();
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Down)),
        AgentPanelAction::Consumed
    );
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Enter)),
        AgentPanelAction::Select(id(2))
    );
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Esc)),
        AgentPanelAction::ReturnToPrompt
    );
    assert!(!panel.is_focused());
}

#[test]
fn updates_preserve_or_safely_clamp_selection() {
    let mut panel = AgentPanel::default();
    panel.set_rows(vec![
        row(1, "one", "running"),
        row(2, "two", "running"),
        row(3, "three", "waiting"),
    ]);
    panel.focus();
    panel.handle_key_event(key(KeyCode::Down));

    panel.set_rows(vec![row(3, "three", "waiting"), row(2, "two", "running")]);
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Enter)),
        AgentPanelAction::Select(id(2))
    );

    panel.set_rows(vec![row(3, "three", "waiting")]);
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Enter)),
        AgentPanelAction::Select(id(3))
    );

    panel.set_rows(Vec::new());
    assert_eq!(panel.desired_height(/*width*/ 60), 0);
    assert!(panel.is_empty());
    assert!(!panel.is_focused());
    assert_eq!(
        panel.handle_key_event(key(KeyCode::Enter)),
        AgentPanelAction::Ignored
    );
}
