//! Root-scoped background refresh for the agent picker.

use super::*;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use std::collections::HashSet;

pub(super) const AGENT_PICKER_VIEW_ID: &str = "agent-picker";
const AGENT_PICKER_PAGE_SIZE: u32 = 100;
const AGENT_PICKER_MAX_THREADS: usize = 1_000;

impl App {
    /// Retain spawn order within each group so activity updates do not move the cursor.
    fn agent_picker_visible_threads(
        &self,
    ) -> Vec<(ThreadId, &crate::multi_agents::AgentPickerThreadEntry)> {
        let mut threads = self.agent_navigation.ordered_threads();
        threads.retain(|(id, _)| {
            self.primary_thread_id == Some(*id)
                || self.agent_navigation.show_completed
                || !self.agent_navigation.monitor.is_successfully_completed(*id)
        });
        threads.sort_by_key(|(id, _)| {
            if self.primary_thread_id == Some(*id) {
                0
            } else if self.agent_navigation.monitor.is_successfully_completed(*id) {
                2
            } else {
                1
            }
        });
        threads
    }

    pub(super) fn agent_picker_selected_thread(&self) -> Option<ThreadId> {
        let index = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID)?;
        self.agent_picker_visible_threads()
            .get(index)
            .map(|(id, _)| *id)
    }

    pub(super) fn repaint_agent_picker(&mut self, selected_thread: Option<ThreadId>) {
        let Some(previous_index) = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID)
        else {
            return;
        };
        let selected = selected_thread
            .and_then(|id| {
                self.agent_picker_visible_threads()
                    .iter()
                    .position(|(candidate, _)| *candidate == id)
            })
            .or_else(|| {
                let visible = self.agent_picker_visible_threads().len();
                let has_toggle = self
                    .agent_navigation
                    .ordered_threads()
                    .iter()
                    .any(|(id, _)| {
                        self.primary_thread_id != Some(*id)
                            && self.agent_navigation.monitor.is_successfully_completed(*id)
                    });
                let count = visible + usize::from(has_toggle);
                (count > 0).then(|| previous_index.min(count - 1))
            });
        let params = self.agent_picker_selection_view_params(selected);
        self.chat_widget
            .replace_selection_view_if_present(AGENT_PICKER_VIEW_ID, params);
    }

    pub(super) fn agent_picker_selection_view_params(
        &self,
        selected: Option<usize>,
    ) -> SelectionViewParams {
        let mut initial_selected_idx = selected;
        let mut items: Vec<SelectionItem> = self
            .agent_picker_visible_threads()
            .into_iter()
            .enumerate()
            .map(|(idx, (thread_id, entry))| {
                if initial_selected_idx.is_none() && self.active_thread_id == Some(thread_id) {
                    initial_selected_idx = Some(idx);
                }
                let id = thread_id;
                let is_primary = self.primary_thread_id == Some(thread_id);
                let name = entry
                    .agent_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|path| !is_primary && !path.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        format_agent_picker_item_name(
                            entry.agent_nickname.as_deref(),
                            entry.agent_role.as_deref(),
                            is_primary,
                        )
                    });
                let summary = self.agent_navigation.monitor.describe(
                    thread_id,
                    entry.is_running,
                    entry.is_closed,
                );
                let cached_model = self
                    .thread_event_channels
                    .get(&thread_id)
                    .and_then(|channel| channel.store.try_lock().ok())
                    .and_then(|store| store.session.as_ref().map(|session| session.model.clone()))
                    .filter(|model| !model.is_empty());
                let model = summary
                    .model
                    .as_deref()
                    .or(cached_model.as_deref())
                    .unwrap_or("model unknown");
                let tokens = summary
                    .total_tokens
                    .map(|total| format!("{total} tokens"))
                    .unwrap_or_else(|| "tokens unavailable".to_string());
                let mut description = format!("{} · {model} · {tokens}", summary.status.label());
                if let Some(activity) = summary.activity {
                    description.push_str(&format!(" · {activity}"));
                }
                SelectionItem {
                    name: name.clone(),
                    description: Some(description),
                    is_current: self.active_thread_id == Some(thread_id),
                    actions: vec![Box::new(move |tx| tx.send(AppEvent::SelectAgentThread(id)))],
                    dismiss_on_select: true,
                    search_value: Some(format!("{name} {thread_id}")),
                    ..Default::default()
                }
            })
            .collect();
        let completed = self
            .agent_navigation
            .ordered_threads()
            .iter()
            .filter(|(id, _)| {
                self.primary_thread_id != Some(*id)
                    && self.agent_navigation.monitor.is_successfully_completed(*id)
            })
            .count();
        if completed > 0 {
            let action = if self.agent_navigation.show_completed {
                "Hide"
            } else {
                "Show"
            };
            items.push(SelectionItem {
                name: format!("{action} completed ({completed})"),
                actions: vec![Box::new(|tx| tx.send(AppEvent::ToggleCompletedAgents))],
                dismiss_on_select: false,
                ..Default::default()
            });
        }
        SelectionViewParams {
            view_id: Some(AGENT_PICKER_VIEW_ID),
            title: Some("Subagents".to_string()),
            subtitle: Some(format!("Live activity. {}", AgentNavigationState::picker_subtitle())),
            footer_note: Some(
                "Tokens are cumulative usage reported by the server."
                    .dim()
                    .into(),
            ),
            footer_hint: Some(standard_popup_hint_line()),
            description_layout:
                crate::bottom_pane::SelectionDescriptionLayout::StackBelowWhenNarrow {
                    min_description_width: 45,
                },
            items,
            initial_selected_idx,
            ..Default::default()
        }
    }

    pub(super) fn refresh_agent_picker_threads(
        &mut self,
        app_server: &AppServerSession,
        root: ThreadId,
    ) {
        let Some(request_id) = self.agent_navigation.begin_picker_refresh(root) else {
            return;
        };
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = async {
                let mut threads = Vec::new();
                let mut cursor = None;
                let mut seen_cursors = HashSet::new();
                while threads.len() < AGENT_PICKER_MAX_THREADS
                    && seen_cursors.insert(cursor.clone())
                {
                    let page = match request_handle
                        .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                            request_id: RequestId::String(Uuid::new_v4().to_string()),
                            params: ThreadListParams {
                                originators: None,
                                cursor,
                                limit: Some(AGENT_PICKER_PAGE_SIZE),
                                sort_key: None,
                                sort_direction: Some(SortDirection::Desc),
                                model_providers: Some(vec![]),
                                source_kinds: Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
                                archived: None,
                                section_id: None,
                                project_id: None,
                                cwd: None,
                                use_state_db_only: true,
                                search_term: None,
                                parent_thread_id: None,
                                ancestor_thread_id: Some(root.to_string()),
                            },
                        })
                        .await
                    {
                        Ok(page) => page,
                        Err(err) if threads.is_empty() => return Err(err.to_string()),
                        Err(err) => {
                            tracing::warn!(%err, "failed to refresh remaining agent picker descendants");
                            break;
                        }
                    };
                    threads.extend(
                        page.data
                            .into_iter()
                            .take(AGENT_PICKER_MAX_THREADS - threads.len()),
                    );
                    let Some(next_cursor) = page.next_cursor else {
                        break;
                    };
                    cursor = Some(next_cursor);
                }
                threads.reverse();
                Ok(threads)
            }
            .await;

            app_event_tx.send(AppEvent::AgentPickerThreadsLoaded {
                primary_thread_id: root,
                request_id,
                result,
            });
        });
    }

    pub(super) fn apply_agent_picker_thread_refresh(
        &mut self,
        root: ThreadId,
        request_id: Uuid,
        result: Result<Vec<Thread>, String>,
    ) {
        if !self
            .agent_navigation
            .finish_picker_refresh(root, request_id)
            || self.primary_thread_id != Some(root)
        {
            return;
        }
        let threads = match result {
            Ok(threads) => threads,
            Err(err) => {
                tracing::warn!(%err, "failed to refresh agent picker descendants");
                return;
            }
        };
        let selected = self.agent_picker_selected_thread();
        for thread in threads {
            let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
                continue;
            };
            self.agent_navigation.monitor.seed(thread_id, &thread);
            let live = self
                .thread_event_channels
                .get(&thread_id)
                .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::Live);
            let previous = self.agent_navigation.get(&thread_id);
            let is_running = matches!(thread.status, ThreadStatus::Active { .. });
            let update_liveness = previous.is_none() || !is_running;
            let is_closed = !live && matches!(thread.status, ThreadStatus::NotLoaded);
            if !is_closed && previous.is_some_and(|entry| entry.is_closed) {
                continue;
            }
            let agent_path = crate::app_server_session::source_agent_path(&thread.source);
            let agent_nickname = thread
                .agent_nickname
                .or_else(|| previous.and_then(|entry| entry.agent_nickname.clone()));
            let agent_role = thread
                .agent_role
                .or_else(|| previous.and_then(|entry| entry.agent_role.clone()));
            if thread.can_accept_direct_input == Some(false) {
                self.agent_navigation.mark_parent_owned(thread_id);
            }
            self.upsert_agent_picker_thread(thread_id, agent_nickname, agent_role, is_closed);
            self.agent_navigation.set_agent_path(thread_id, agent_path);
            if !live && update_liveness {
                self.agent_navigation.set_running(thread_id, is_running);
            }
        }

        self.repaint_agent_picker(selected);
    }
}
