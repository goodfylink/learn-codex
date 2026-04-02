use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::watch;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Closed,
}

impl AgentStatus {
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Closed
        )
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AgentSnapshot {
    pub id: String,
    pub role: String,
    pub status: AgentStatus,
    pub history_items: usize,
    pub pending_inputs: usize,
}

pub struct AgentThread {
    id: String,
    role: String,
    history: Mutex<Vec<Value>>,
    pending_inputs: Mutex<Vec<String>>,
    status: Mutex<AgentStatus>,
    status_tx: watch::Sender<AgentStatus>,
    last_result: Mutex<Option<String>>,
    last_error: Mutex<Option<String>>,
    worker_active: AtomicBool,
    closed: AtomicBool,
}

impl AgentThread {
    pub fn new(id: String, role: String, system_prompt: String) -> Self {
        let initial_status = AgentStatus::Pending;
        let (status_tx, _) = watch::channel(initial_status.clone());

        Self {
            id,
            role,
            history: Mutex::new(vec![serde_json::json!({
                "role": "system",
                "content": system_prompt,
            })]),
            pending_inputs: Mutex::new(Vec::new()),
            status: Mutex::new(initial_status),
            status_tx,
            last_result: Mutex::new(None),
            last_error: Mutex::new(None),
            worker_active: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn enqueue_input(&self, input: String) {
        let mut pending_inputs = self
            .pending_inputs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending_inputs.push(input);
    }

    pub fn take_next_input(&self) -> Option<String> {
        let mut pending_inputs = self
            .pending_inputs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending_inputs.is_empty() {
            None
        } else {
            Some(pending_inputs.remove(0))
        }
    }

    pub fn has_pending_inputs(&self) -> bool {
        !self
            .pending_inputs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    }

    pub fn try_start_worker(&self) -> bool {
        self.worker_active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn mark_worker_stopped(&self) {
        self.worker_active.store(false, Ordering::SeqCst);
    }

    pub fn set_status(&self, status: AgentStatus) {
        let mut current = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = status.clone();
        let _ = self.status_tx.send(status);
    }

    pub fn status(&self) -> AgentStatus {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.status_tx.subscribe()
    }

    pub fn push_history_item(&self, item: Value) {
        let mut history = self
            .history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        history.push(item);
    }

    pub fn history_snapshot(&self) -> Vec<Value> {
        self.history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn set_last_result(&self, result: String) {
        let mut last_result = self
            .last_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *last_result = Some(result);
        let mut last_error = self
            .last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *last_error = None;
    }

    pub fn set_last_error(&self, error: String) {
        let mut last_error = self
            .last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *last_error = Some(error);
    }

    pub fn last_result(&self) -> Option<String> {
        self.last_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.set_status(AgentStatus::Closed);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn snapshot(&self) -> AgentSnapshot {
        let history_items = self
            .history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len();
        let pending_inputs = self
            .pending_inputs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len();

        AgentSnapshot {
            id: self.id.clone(),
            role: self.role.clone(),
            status: self.status(),
            history_items,
            pending_inputs,
        }
    }
}

pub struct AgentTeamManager {
    next_agent_id: AtomicU64,
    agents: Mutex<HashMap<String, Arc<AgentThread>>>,
}

impl AgentTeamManager {
    pub fn new() -> Self {
        Self {
            next_agent_id: AtomicU64::new(1),
            agents: Mutex::new(HashMap::new()),
        }
    }

    pub fn spawn_agent(
        &self,
        role: &str,
        system_prompt: &str,
        initial_input: &str,
    ) -> Arc<AgentThread> {
        let id = format!(
            "agent-{}",
            self.next_agent_id.fetch_add(1, Ordering::Relaxed)
        );
        let agent = Arc::new(AgentThread::new(
            id.clone(),
            role.to_string(),
            system_prompt.to_string(),
        ));
        agent.enqueue_input(initial_input.to_string());

        let mut agents = self
            .agents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        agents.insert(id, Arc::clone(&agent));
        agent
    }

    pub fn get(&self, id: &str) -> Option<Arc<AgentThread>> {
        self.agents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(id)
            .cloned()
    }

    pub fn list_snapshots(&self) -> Vec<AgentSnapshot> {
        let agents = self
            .agents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut snapshots = agents
            .values()
            .map(|agent| agent.snapshot())
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.id.cmp(&right.id));
        snapshots
    }
}

#[cfg(test)]
mod tests {
    use super::AgentSnapshot;
    use super::AgentStatus;
    use super::AgentTeamManager;

    #[test]
    fn agent_thread_queues_inputs_and_tracks_terminal_state() {
        let manager = AgentTeamManager::new();
        let agent = manager.spawn_agent("worker", "system prompt", "initial task");

        assert_eq!(agent.status(), AgentStatus::Pending);
        assert_eq!(agent.take_next_input(), Some("initial task".to_string()));
        assert_eq!(agent.take_next_input(), None);

        agent.enqueue_input("follow-up task".to_string());
        assert!(agent.has_pending_inputs());
        assert_eq!(agent.take_next_input(), Some("follow-up task".to_string()));

        agent.set_last_result("done".to_string());
        assert_eq!(agent.last_result(), Some("done".to_string()));
        assert_eq!(agent.last_error(), None);

        agent.close();
        assert_eq!(agent.status(), AgentStatus::Closed);
        assert!(agent.status().is_final());
        assert!(agent.is_closed());
    }

    #[test]
    fn manager_lists_sorted_agent_snapshots() {
        let manager = AgentTeamManager::new();
        let first = manager.spawn_agent("default", "system", "task one");
        let second = manager.spawn_agent("reviewer", "system", "task two");

        let snapshots = manager.list_snapshots();

        assert_eq!(
            snapshots,
            vec![
                AgentSnapshot {
                    id: first.id().to_string(),
                    role: "default".to_string(),
                    status: AgentStatus::Pending,
                    history_items: 1,
                    pending_inputs: 1,
                },
                AgentSnapshot {
                    id: second.id().to_string(),
                    role: "reviewer".to_string(),
                    status: AgentStatus::Pending,
                    history_items: 1,
                    pending_inputs: 1,
                },
            ]
        );
    }
}
