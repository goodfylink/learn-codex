use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentSnapshot {
    pub id: String,
    pub role: String,
    pub status: AgentStatus,
    pub history_items: usize,
}

pub struct AgentThread {
    id: String,
    role: String,
    history: Mutex<Vec<Value>>,
    status: Mutex<AgentStatus>,
}

impl AgentThread {
    pub fn new(id: String, role: String, history: Vec<Value>) -> Self {
        Self {
            id,
            role,
            history: Mutex::new(history),
            status: Mutex::new(AgentStatus::Pending),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn set_status(&self, status: AgentStatus) {
        let mut current = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = status;
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

    pub fn snapshot(&self) -> AgentSnapshot {
        let history_items = self
            .history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len();
        let status = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();

        AgentSnapshot {
            id: self.id.clone(),
            role: self.role.clone(),
            status,
            history_items,
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
        instruction: &str,
    ) -> Arc<AgentThread> {
        let id = format!(
            "agent-{}",
            self.next_agent_id.fetch_add(1, Ordering::Relaxed)
        );
        let history = vec![
            json!({
                "role": "system",
                "content": system_prompt,
            }),
            json!({
                "role": "user",
                "content": instruction,
            }),
        ];
        let agent = Arc::new(AgentThread::new(id.clone(), role.to_string(), history));

        let mut agents = self
            .agents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        agents.insert(id, Arc::clone(&agent));
        agent
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
