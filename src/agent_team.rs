use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Default,
    Explorer,
    Worker,
}

impl AgentRole {
    pub fn parse(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "default" => Some(Self::Default),
            "explorer" => Some(Self::Explorer),
            "worker" => Some(Self::Worker),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Explorer => "explorer",
            Self::Worker => "worker",
        }
    }

    pub fn system_prompt(self) -> &'static str {
        match self {
            Self::Default => {
                "You are a delegated agent assisting a primary agent. Use tools responsibly and complete the assigned task efficiently."
            }
            Self::Explorer => {
                "You are an explorer agent. Focus on inspection, reading files, and gathering evidence. Do not modify files or perform mutating shell actions."
            }
            Self::Worker => {
                "You are a worker agent. Execute a bounded implementation task, use tools carefully, and leave a clear result for the parent agent."
            }
        }
    }

    pub fn allows_file_mutation(self) -> bool {
        !matches!(self, Self::Explorer)
    }
}

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
    pub role: AgentRole,
    pub parent_agent_id: Option<String>,
    pub depth: usize,
    pub status: AgentStatus,
    pub history_items: usize,
    pub pending_inputs: usize,
}

pub struct AgentSpawnRequest {
    pub role: AgentRole,
    pub parent_agent_id: Option<String>,
    pub depth: usize,
    pub initial_history: Vec<Value>,
    pub initial_input: String,
}

pub struct AgentThread {
    id: String,
    role: AgentRole,
    parent_agent_id: Option<String>,
    depth: usize,
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
    pub fn new(id: String, request: AgentSpawnRequest) -> Self {
        let initial_status = AgentStatus::Pending;
        let (status_tx, _) = watch::channel(initial_status.clone());

        Self {
            id,
            role: request.role,
            parent_agent_id: request.parent_agent_id,
            depth: request.depth,
            history: Mutex::new(request.initial_history),
            pending_inputs: Mutex::new(vec![request.initial_input]),
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

    pub fn role(&self) -> AgentRole {
        self.role
    }

    pub fn parent_agent_id(&self) -> Option<String> {
        self.parent_agent_id.clone()
    }

    pub fn depth(&self) -> usize {
        self.depth
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
            role: self.role,
            parent_agent_id: self.parent_agent_id.clone(),
            depth: self.depth,
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

    pub fn spawn_agent(&self, request: AgentSpawnRequest) -> Arc<AgentThread> {
        let id = format!(
            "agent-{}",
            self.next_agent_id.fetch_add(1, Ordering::Relaxed)
        );
        let agent = Arc::new(AgentThread::new(id.clone(), request));

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
    use super::AgentRole;
    use super::AgentSnapshot;
    use super::AgentSpawnRequest;
    use super::AgentStatus;
    use super::AgentTeamManager;
    use serde_json::json;

    #[test]
    fn agent_thread_queues_inputs_and_tracks_terminal_state() {
        let manager = AgentTeamManager::new();
        let agent = manager.spawn_agent(AgentSpawnRequest {
            role: AgentRole::Worker,
            parent_agent_id: Some("agent-parent".to_string()),
            depth: 2,
            initial_history: vec![json!({
                "role": "system",
                "content": "system prompt",
            })],
            initial_input: "initial task".to_string(),
        });

        assert_eq!(agent.role(), AgentRole::Worker);
        assert_eq!(agent.parent_agent_id(), Some("agent-parent".to_string()));
        assert_eq!(agent.depth(), 2);
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
        let first = manager.spawn_agent(AgentSpawnRequest {
            role: AgentRole::Default,
            parent_agent_id: None,
            depth: 1,
            initial_history: vec![json!({
                "role": "system",
                "content": "system",
            })],
            initial_input: "task one".to_string(),
        });
        let second = manager.spawn_agent(AgentSpawnRequest {
            role: AgentRole::Explorer,
            parent_agent_id: Some(first.id().to_string()),
            depth: 2,
            initial_history: vec![json!({
                "role": "system",
                "content": "system",
            })],
            initial_input: "task two".to_string(),
        });

        let snapshots = manager.list_snapshots();

        assert_eq!(
            snapshots,
            vec![
                AgentSnapshot {
                    id: first.id().to_string(),
                    role: AgentRole::Default,
                    parent_agent_id: None,
                    depth: 1,
                    status: AgentStatus::Pending,
                    history_items: 1,
                    pending_inputs: 1,
                },
                AgentSnapshot {
                    id: second.id().to_string(),
                    role: AgentRole::Explorer,
                    parent_agent_id: Some(first.id().to_string()),
                    depth: 2,
                    status: AgentStatus::Pending,
                    history_items: 1,
                    pending_inputs: 1,
                },
            ]
        );
    }

    #[test]
    fn role_parser_accepts_supported_labels() {
        assert_eq!(AgentRole::parse("default"), Some(AgentRole::Default));
        assert_eq!(AgentRole::parse("explorer"), Some(AgentRole::Explorer));
        assert_eq!(AgentRole::parse("worker"), Some(AgentRole::Worker));
        assert_eq!(AgentRole::parse("unknown"), None);
    }
}
