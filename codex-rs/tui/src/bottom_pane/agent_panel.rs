use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::render::renderable::Renderable;

const MAX_VISIBLE_ROWS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentPanelRow {
    pub(crate) thread_id: ThreadId,
    pub(crate) name: String,
    pub(crate) detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentPanelAction {
    Select(ThreadId),
    Consumed,
    ReturnToPrompt,
    Ignored,
}

/// Persistent, keyboard-focusable list of active child agents below the composer.
#[derive(Default)]
pub(crate) struct AgentPanel {
    rows: Vec<AgentPanelRow>,
    selected_thread_id: Option<ThreadId>,
    focused: bool,
}

impl AgentPanel {
    pub(crate) fn set_rows(&mut self, rows: Vec<AgentPanelRow>) -> bool {
        if self.rows == rows {
            return false;
        }

        let previous_index = self
            .selected_thread_id
            .and_then(|selected| {
                self.rows
                    .iter()
                    .position(|row| row.thread_id == selected)
            })
            .unwrap_or(0);
        let selected_thread_id = self.selected_thread_id.filter(|selected| {
            rows.iter().any(|row| row.thread_id == *selected)
        });

        self.rows = rows;
        self.selected_thread_id = selected_thread_id.or_else(|| {
            let index = previous_index.min(self.rows.len().saturating_sub(1));
            self.rows.get(index).map(|row| row.thread_id)
        });
        if self.rows.is_empty() {
            self.focused = false;
        }
        true
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn focus(&mut self) {
        if !self.rows.is_empty() {
            self.focused = true;
        }
    }

    pub(crate) fn unfocus(&mut self) {
        self.focused = false;
    }

    pub(crate) fn is_focused(&self) -> bool {
        self.focused
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) -> AgentPanelAction {
        if !self.focused
            || !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            return AgentPanelAction::Ignored;
        }

        match key_event.code {
            KeyCode::Up => {
                self.move_selection(-1);
                AgentPanelAction::Consumed
            }
            KeyCode::Down => {
                self.move_selection(1);
                AgentPanelAction::Consumed
            }
            KeyCode::Enter => self
                .selected_thread_id
                .map(AgentPanelAction::Select)
                .unwrap_or(AgentPanelAction::Consumed),
            KeyCode::Esc => {
                self.unfocus();
                AgentPanelAction::ReturnToPrompt
            }
            _ => AgentPanelAction::Ignored,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let Some(current) = self.selected_index() else {
            return;
        };
        let last = self.rows.len().saturating_sub(1);
        let next = current.saturating_add_signed(delta).min(last);
        self.selected_thread_id = Some(self.rows[next].thread_id);
    }

    fn selected_index(&self) -> Option<usize> {
        let selected = self.selected_thread_id?;
        self.rows
            .iter()
            .position(|row| row.thread_id == selected)
    }

    fn render_lines(&self, height: u16) -> Vec<Line<'static>> {
        if self.rows.is_empty() || height == 0 {
            return Vec::new();
        }

        let hint = if self.focused {
            "  ↑↓ navigate  enter open  esc prompt"
        } else {
            "  ↓ browse  /subagents all"
        };
        let mut lines = vec![Line::from(vec!["  Active agents".bold(), hint.dim()])];
        let visible_rows = MAX_VISIBLE_ROWS.min(usize::from(height.saturating_sub(1)));
        if visible_rows == 0 {
            return lines;
        }
        let selected_index = self.selected_index().unwrap_or(0);
        let start = selected_index
            .saturating_add(1)
            .saturating_sub(visible_rows)
            .min(self.rows.len().saturating_sub(visible_rows));

        for (index, row) in self.rows.iter().enumerate().skip(start).take(visible_rows) {
            let selected = self.focused && index == selected_index;
            let marker = if selected { "› ".cyan().bold() } else { "  ".into() };
            let name = if selected {
                row.name.clone().cyan().bold()
            } else {
                row.name.clone().into()
            };
            lines.push(Line::from(vec![
                marker,
                name,
                format!("  {}", row.detail).dim(),
            ]));
        }
        lines
    }
}

impl Renderable for AgentPanel {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        Paragraph::new(self.render_lines(area.height)).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        if width == 0 || self.rows.is_empty() {
            0
        } else {
            1 + self.rows.len().min(MAX_VISIBLE_ROWS) as u16
        }
    }
}

#[cfg(test)]
#[path = "agent_panel_tests.rs"]
mod tests;
