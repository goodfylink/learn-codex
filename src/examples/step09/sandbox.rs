use crate::agent_team::AgentRole;
use crate::agent_team::AgentSpawnRequest;
use crate::agent_team::AgentStatus;
use crate::agent_team::AgentTeamManager;
use crate::agent_team::AgentThread;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::timeout;

// The registry handle is threaded through handlers so delegated execution can
// re-enter the same tool dispatcher.

/// Minimal tool handler abstraction used by the demo runtime.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn name(&self) -> &str;
    fn spec(&self) -> serde_json::Value;
    fn supports_parallel_tool_calls(&self) -> bool {
        false
    }
    fn requires_dispatch_lock(&self) -> bool {
        true
    }
    // The shared registry handle allows delegated execution to re-enter the dispatcher.
    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> String;
}

#[derive(Clone, Debug, Default)]
pub struct ToolExecutionContext {
    pub caller_agent_id: Option<String>,
    pub caller_role: Option<AgentRole>,
    pub caller_depth: usize,
}

impl ToolExecutionContext {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn for_agent(agent: &AgentThread) -> Self {
        Self {
            caller_agent_id: Some(agent.id().to_string()),
            caller_role: Some(agent.role()),
            caller_depth: agent.depth(),
        }
    }
}

pub struct ToolInvocation {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub context: ToolExecutionContext,
}

pub struct ToolInvocationResult {
    pub call_id: String,
    pub tool_name: String,
    pub output: String,
}

/// Registry for tool handlers and shared per-session state.
pub struct ToolRegistry {
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
    plan_state: Mutex<Vec<PlanItem>>,
    parallel_execution: RwLock<()>,
    agent_team: AgentTeamManager,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            plan_state: Mutex::new(Vec::new()),
            parallel_execution: RwLock::new(()),
            agent_team: AgentTeamManager::new(),
        }
    }

    pub fn register(&mut self, handler: Arc<dyn ToolHandler>) {
        println!("[registry] registering tool: {}", handler.name());
        self.handlers.insert(handler.name().to_string(), handler);
    }

    pub async fn dispatch(
        self: Arc<Self>,
        name: &str,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> Option<String> {
        let handler = self.handlers.get(name).cloned()?;
        println!("[registry] dispatching tool: {}", name);

        if !handler.requires_dispatch_lock() {
            return Some(handler.handle(self.clone(), context, arguments).await);
        }

        if handler.supports_parallel_tool_calls() {
            let _guard = self.parallel_execution.read().await;
            Some(handler.handle(self.clone(), context, arguments).await)
        } else {
            let _guard = self.parallel_execution.write().await;
            Some(handler.handle(self.clone(), context, arguments).await)
        }
    }

    pub async fn dispatch_many(
        self: Arc<Self>,
        invocations: Vec<ToolInvocation>,
    ) -> Vec<ToolInvocationResult> {
        let mut results = std::iter::repeat_with(|| None)
            .take(invocations.len())
            .collect::<Vec<Option<ToolInvocationResult>>>();
        let mut handles = Vec::with_capacity(invocations.len());

        for (index, invocation) in invocations.into_iter().enumerate() {
            let registry = Arc::clone(&self);
            let fallback_call_id = invocation.call_id.clone();
            let fallback_tool_name = invocation.tool_name.clone();
            let fallback_arguments = invocation.arguments.clone();
            let handle = tokio::spawn(async move {
                let output = registry
                    .clone()
                    .dispatch(
                        &invocation.tool_name,
                        invocation.context.clone(),
                        &invocation.arguments,
                    )
                    .await
                    .unwrap_or_else(|| {
                        tool_not_found_output(
                            invocation.tool_name.as_str(),
                            invocation.arguments.as_str(),
                        )
                    });
                ToolInvocationResult {
                    call_id: invocation.call_id,
                    tool_name: invocation.tool_name,
                    output,
                }
            });
            handles.push((
                index,
                ToolInvocationResult {
                    call_id: fallback_call_id,
                    tool_name: fallback_tool_name.clone(),
                    output: tool_error(
                        fallback_tool_name.as_str(),
                        "tool_task_failed",
                        "tool task failed before producing a result",
                        json!({
                            "arguments": fallback_arguments,
                        }),
                    ),
                },
                handle,
            ));
        }

        for (index, fallback, handle) in handles {
            results[index] = Some(match handle.await {
                Ok(result) => result,
                Err(_) => fallback,
            });
        }

        results.into_iter().flatten().collect()
    }

    pub fn get_specs(&self) -> Vec<serde_json::Value> {
        self.handlers.values().map(|h| h.spec()).collect()
    }

    pub fn spawn_agent_thread(&self, request: AgentSpawnRequest) -> Arc<AgentThread> {
        self.agent_team.spawn_agent(request)
    }

    pub fn get_agent_thread(&self, id: &str) -> Option<Arc<AgentThread>> {
        self.agent_team.get(id)
    }

    pub fn agent_snapshots(&self) -> Vec<crate::agent_team::AgentSnapshot> {
        self.agent_team.list_snapshots()
    }

    fn update_plan_state(&self, plan: &[PlanItem]) -> Result<Vec<PlanItem>, String> {
        if let Some(item) = plan.iter().find(|item| !item.has_valid_status()) {
            return Err(format!(
                "invalid plan status '{}' for step '{}'",
                item.status, item.step
            ));
        }

        let in_progress_count = plan
            .iter()
            .filter(|item| item.status == "in_progress")
            .count();
        if in_progress_count > 1 {
            return Err("plan can contain at most one in_progress step".to_string());
        }

        let mut current_plan = self
            .plan_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current_plan = plan.to_vec();
        Ok(current_plan.clone())
    }
}

const MAX_TOOL_CONTENT_CHARS: usize = 100_000;
const BASH_OUTPUT_CHARS_PER_STREAM: usize = 40_000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const DEMO_SUPPORTS_PARALLEL_TOOL_CALLS: bool = true;
const MAX_AGENT_DEPTH: usize = 2;

#[derive(Serialize)]
struct ToolResult {
    ok: bool,
    tool: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    data: serde_json::Value,
}

#[derive(Serialize)]
struct TruncatedText {
    content: String,
    truncated: bool,
    omitted_chars: usize,
}

fn serialize_tool_result(tool_result: ToolResult) -> String {
    serde_json::to_string_pretty(&tool_result).unwrap_or_else(|e| {
        json!({
            "ok": false,
            "tool": "internal",
            "message": format!("failed to serialize tool result: {e}"),
            "error_code": "serialization_failed",
            "data": {},
        })
        .to_string()
    })
}

fn tool_success(tool: &str, message: impl Into<String>, data: serde_json::Value) -> String {
    serialize_tool_result(ToolResult {
        ok: true,
        tool: tool.to_string(),
        message: message.into(),
        error_code: None,
        data,
    })
}

fn tool_error(
    tool: &str,
    error_code: &str,
    message: impl Into<String>,
    data: serde_json::Value,
) -> String {
    serialize_tool_result(ToolResult {
        ok: false,
        tool: tool.to_string(),
        message: message.into(),
        error_code: Some(error_code.to_string()),
        data,
    })
}

fn tool_not_found_output(tool: &str, arguments: &str) -> String {
    tool_error(
        tool,
        "tool_not_found",
        format!("tool {tool} not found"),
        json!({
            "arguments": arguments,
        }),
    )
}

#[derive(Clone)]
struct AgentExecutionConfig {
    api_key: String,
    base_url: String,
    model_name: String,
}

const DEFAULT_WAIT_AGENT_TIMEOUT: Duration = Duration::from_secs(30);

fn build_agent_history(
    role: AgentRole,
    parent: Option<&AgentThread>,
    fork_context: bool,
) -> (Vec<serde_json::Value>, bool) {
    let mut history = vec![json!({
        "role": "system",
        "content": role.system_prompt(),
    })];

    let Some(parent) = parent else {
        return (history, false);
    };

    if !fork_context {
        return (history, false);
    }

    let parent_history = parent.history_snapshot();
    history.extend(parent_history.into_iter().skip(1));
    history.push(json!({
        "role": "system",
        "content": format!(
            "You were spawned by parent agent {} as role {}. The inherited conversation context appears above.",
            parent.id(),
            role.label(),
        ),
    }));
    (history, true)
}

fn mutation_not_allowed(
    tool: &str,
    context: &ToolExecutionContext,
    reason: &str,
) -> Option<String> {
    if let Some(role) = context.caller_role {
        if !role.allows_file_mutation() {
            return Some(tool_error(
                tool,
                "role_violation",
                reason,
                json!({
                    "caller_agent_id": context.caller_agent_id,
                    "caller_role": context.caller_role,
                }),
            ));
        }
    }

    None
}

fn command_looks_mutating(cmd: &str) -> bool {
    let normalized = cmd.trim().to_ascii_lowercase();
    [
        "rm ",
        "mv ",
        "cp ",
        "mkdir ",
        "touch ",
        "chmod ",
        "chown ",
        "tee ",
        "sed -i",
        "perl -i",
        "patch ",
        "git apply",
        "git commit",
        "cargo add",
        "npm install",
        "pip install",
        " >",
        " >>",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}

fn notify_parent_of_agent_update(
    registry: &ToolRegistry,
    agent: &AgentThread,
    status: AgentStatus,
) {
    let Some(parent_id) = agent.parent_agent_id() else {
        return;
    };
    let Some(parent) = registry.get_agent_thread(&parent_id) else {
        return;
    };

    let content = match status {
        AgentStatus::Completed => format!(
            "Child agent {} [{}] completed its task.\nResult:\n{}",
            agent.id(),
            agent.role().label(),
            agent
                .last_result()
                .unwrap_or_else(|| "No result".to_string()),
        ),
        AgentStatus::Failed => format!(
            "Child agent {} [{}] failed.\nError:\n{}",
            agent.id(),
            agent.role().label(),
            agent
                .last_error()
                .unwrap_or_else(|| "Unknown error".to_string()),
        ),
        AgentStatus::Closed => format!(
            "Child agent {} [{}] was closed before completion.",
            agent.id(),
            agent.role().label(),
        ),
        AgentStatus::Pending | AgentStatus::Running => return,
    };

    parent.push_history_item(json!({
        "role": "system",
        "content": content,
    }));
    println!(
        "[agent-team] notified parent {} about child {}",
        parent_id,
        agent.id()
    );
}

fn start_agent_worker(
    registry: Arc<ToolRegistry>,
    agent: Arc<AgentThread>,
    config: AgentExecutionConfig,
) {
    if !agent.try_start_worker() {
        return;
    }

    tokio::spawn(async move {
        loop {
            if agent.is_closed() {
                notify_parent_of_agent_update(&registry, &agent, AgentStatus::Closed);
                agent.mark_worker_stopped();
                break;
            }

            let Some(instruction) = agent.take_next_input() else {
                agent.mark_worker_stopped();
                if agent.has_pending_inputs() && agent.try_start_worker() {
                    continue;
                }
                break;
            };

            agent.set_status(AgentStatus::Running);
            agent.push_history_item(json!({
                "role": "user",
                "content": instruction,
            }));

            match run_agent_turn(Arc::clone(&registry), Arc::clone(&agent), config.clone()).await {
                Ok(final_content) => {
                    if !agent.is_closed() {
                        agent.set_last_result(final_content);
                        agent.set_status(AgentStatus::Completed);
                        notify_parent_of_agent_update(&registry, &agent, AgentStatus::Completed);
                    }
                }
                Err(err) => {
                    if !agent.is_closed() {
                        agent.set_last_error(err);
                        agent.set_status(AgentStatus::Failed);
                        notify_parent_of_agent_update(&registry, &agent, AgentStatus::Failed);
                    }
                }
            }
        }
    });
}

async fn run_agent_turn(
    registry: Arc<ToolRegistry>,
    agent: Arc<AgentThread>,
    config: AgentExecutionConfig,
) -> Result<String, String> {
    let client = Client::new();
    let tool_specs = registry.get_specs();

    loop {
        if agent.is_closed() {
            return Err("agent closed before completion".to_string());
        }

        let history = agent.history_snapshot();
        let payload = json!({
            "model": config.model_name,
            "messages": history,
            "tools": tool_specs,
            "parallel_tool_calls": DEMO_SUPPORTS_PARALLEL_TOOL_CALLS,
            "temperature": 0.2
        });

        let res = client
            .post(&config.base_url)
            .bearer_auth(&config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("sub-agent API call failed: {e}"))?;

        let response_json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("failed to parse sub-agent response: {e}"))?;

        let choice = &response_json["choices"][0];
        let message = &choice["message"];
        agent.push_history_item(message.clone());

        if choice["finish_reason"] == "tool_calls" || message["tool_calls"].is_array() {
            let tool_calls = message["tool_calls"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            println!(
                "[agent-team] {} requested {} tool call(s)",
                agent.id(),
                tool_calls.len()
            );

            let invocations = tool_calls
                .iter()
                .filter_map(|tool_call| {
                    let function_name = tool_call["function"]["name"].as_str()?;
                    let arguments = tool_call["function"]["arguments"].as_str()?;
                    let call_id = tool_call["id"].as_str()?;
                    println!(
                        "[agent-team] {} tool request: {}({})",
                        agent.id(),
                        function_name,
                        arguments
                    );
                    Some(ToolInvocation {
                        call_id: call_id.to_string(),
                        tool_name: function_name.to_string(),
                        arguments: arguments.to_string(),
                        context: ToolExecutionContext::for_agent(&agent),
                    })
                })
                .collect::<Vec<_>>();

            let tool_outputs = registry.clone().dispatch_many(invocations).await;
            for tool_output in tool_outputs {
                agent.push_history_item(json!({
                    "role": "tool",
                    "content": tool_output.output,
                    "tool_call_id": tool_output.call_id,
                }));
            }
            continue;
        }

        return Ok(message["content"].as_str().unwrap_or("Done").to_string());
    }
}

async fn wait_for_agent_status(
    agent: Arc<AgentThread>,
    timeout_duration: Duration,
) -> (AgentStatus, bool) {
    let current_status = agent.status();
    if current_status.is_final() {
        return (current_status, false);
    }

    let mut status_rx = agent.subscribe_status();
    let wait_result = timeout(timeout_duration, async {
        loop {
            if status_rx.changed().await.is_err() {
                break agent.status();
            }
            let status = status_rx.borrow().clone();
            if status.is_final() {
                break status;
            }
        }
    })
    .await;

    match wait_result {
        Ok(status) => (status, false),
        Err(_) => (agent.status(), true),
    }
}

// Tool handler implementations.

pub struct RunBashHandler;

#[derive(Deserialize)]
struct RunBashArgs {
    cmd: String,
}

#[async_trait]
impl ToolHandler for RunBashHandler {
    fn name(&self) -> &str {
        "run_bash"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Execute a bash command on the user's machine to list files, read code, or make changes.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "cmd": {
                            "type": "string",
                            "description": "The bash command string to execute (e.g., 'pwd', 'ls -la', 'cat file.rs')"
                        }
                    },
                    "required": ["cmd"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        _registry: Arc<ToolRegistry>,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        if let Ok(args) = serde_json::from_str::<RunBashArgs>(arguments) {
            if command_looks_mutating(&args.cmd) {
                if let Some(error) = mutation_not_allowed(
                    self.name(),
                    &context,
                    "explorer agents may only run read-only shell commands",
                ) {
                    return error;
                }
            }
            execute_bash(&args.cmd)
        } else {
            tool_error(
                self.name(),
                "invalid_arguments",
                format!("failed to parse arguments for {}", self.name()),
                json!({ "arguments": arguments }),
            )
        }
    }
}

pub struct ReadFileHandler;

#[derive(Deserialize)]
struct ReadFileArgs {
    path: String,
}

#[async_trait]
impl ToolHandler for ReadFileHandler {
    fn name(&self) -> &str {
        "read_file"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Read the content of a file from the disk.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the file."
                        }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        _registry: Arc<ToolRegistry>,
        _context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        if let Ok(args) = serde_json::from_str::<ReadFileArgs>(arguments) {
            read_file(&args.path)
        } else {
            tool_error(
                self.name(),
                "invalid_arguments",
                format!("failed to parse arguments for {}", self.name()),
                json!({ "arguments": arguments }),
            )
        }
    }
}

pub struct WriteFileHandler;

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[async_trait]
impl ToolHandler for WriteFileHandler {
    fn name(&self) -> &str {
        "write_file"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Create or overwrite a file with new content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path where the file should be written."
                        },
                        "content": {
                            "type": "string",
                            "description": "The full content to write into the file."
                        }
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        _registry: Arc<ToolRegistry>,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        if let Some(error) = mutation_not_allowed(
            self.name(),
            &context,
            "explorer agents are not allowed to write files",
        ) {
            return error;
        }
        if let Ok(args) = serde_json::from_str::<WriteFileArgs>(arguments) {
            write_file(&args.path, &args.content)
        } else {
            tool_error(
                self.name(),
                "invalid_arguments",
                format!("failed to parse arguments for {}", self.name()),
                json!({ "arguments": arguments }),
            )
        }
    }
}

pub struct EditFileHandler;

#[derive(Deserialize)]
struct EditFileArgs {
    path: String,
    target: String,
    replacement: String,
}

#[async_trait]
impl ToolHandler for EditFileHandler {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Edit an existing file by replacing a target string with a new string.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to edit."
                        },
                        "target": {
                            "type": "string",
                            "description": "The exact string within the file to be replaced."
                        },
                        "replacement": {
                            "type": "string",
                            "description": "The new string to insert instead of the target."
                        }
                    },
                    "required": ["path", "target", "replacement"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        _registry: Arc<ToolRegistry>,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        if let Some(error) = mutation_not_allowed(
            self.name(),
            &context,
            "explorer agents are not allowed to edit files",
        ) {
            return error;
        }
        if let Ok(args) = serde_json::from_str::<EditFileArgs>(arguments) {
            edit_file(&args.path, &args.target, &args.replacement)
        } else {
            tool_error(
                self.name(),
                "invalid_arguments",
                format!("failed to parse arguments for {}", self.name()),
                json!({ "arguments": arguments }),
            )
        }
    }
}

pub struct PlanHandler;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlanItem {
    pub step: String,
    pub status: String, // pending, in_progress, completed
}

impl PlanItem {
    fn has_valid_status(&self) -> bool {
        matches!(
            self.status.as_str(),
            "pending" | "in_progress" | "completed"
        )
    }
}

#[derive(Deserialize, Debug)]
pub struct UpdatePlanArgs {
    pub explanation: Option<String>,
    pub plan: Vec<PlanItem>,
}

#[async_trait]
impl ToolHandler for PlanHandler {
    fn name(&self) -> &str {
        "update_plan"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Updates the task plan. Use this at the start of complex tasks to decompose them into steps, and keep it updated as you progress.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "explanation": {
                            "type": "string",
                            "description": "An optional explanation for the plan change."
                        },
                        "plan": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "step": {
                                        "type": "string"
                                    },
                                    "status": {
                                        "type": "string",
                                        "enum": ["pending", "in_progress", "completed"]
                                    }
                                },
                                "required": ["step", "status"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["plan"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        _context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        if let Ok(args) = serde_json::from_str::<UpdatePlanArgs>(arguments) {
            let current_plan = match registry.update_plan_state(&args.plan) {
                Ok(current_plan) => current_plan,
                Err(e) => {
                    return tool_error(
                        self.name(),
                        "invalid_plan",
                        format!("plan update rejected: {e}"),
                        json!({
                            "explanation": args.explanation.clone(),
                            "plan": args.plan.clone(),
                        }),
                    )
                }
            };

            println!("\n[plan] update received");
            if let Some(exp) = &args.explanation {
                println!("Explanation: {}", exp);
            }
            for item in &current_plan {
                println!("  - {} [{}]", item.step, item.status);
            }
            println!();

            tool_success(
                self.name(),
                "plan updated successfully",
                json!({
                    "explanation": args.explanation.clone(),
                    "plan": current_plan,
                }),
            )
        } else {
            tool_error(
                self.name(),
                "invalid_arguments",
                format!("failed to parse arguments for {}", self.name()),
                json!({ "arguments": arguments }),
            )
        }
    }
}

// Multi-agent collaboration tools.

fn resolve_agent_role(role_label: Option<&str>) -> Result<AgentRole, String> {
    let role_label = role_label.unwrap_or("default");
    AgentRole::parse(role_label).ok_or_else(|| {
        format!("unsupported role '{role_label}'; expected one of: default, explorer, worker")
    })
}

fn spawn_delegated_agent(
    registry: &ToolRegistry,
    context: &ToolExecutionContext,
    role: AgentRole,
    instruction: &str,
    fork_context: bool,
) -> Result<(Arc<AgentThread>, bool), String> {
    let parent = context
        .caller_agent_id
        .as_deref()
        .and_then(|agent_id| registry.get_agent_thread(agent_id));
    let depth = if parent.is_some() {
        context.caller_depth + 1
    } else {
        1
    };
    if depth > MAX_AGENT_DEPTH {
        return Err(format!(
            "agent depth limit exceeded: requested depth {depth}, maximum is {MAX_AGENT_DEPTH}"
        ));
    }

    let (initial_history, fork_applied) =
        build_agent_history(role, parent.as_deref(), fork_context);
    let agent = registry.spawn_agent_thread(AgentSpawnRequest {
        role,
        parent_agent_id: parent.as_ref().map(|agent| agent.id().to_string()),
        depth,
        initial_history,
        initial_input: instruction.to_string(),
    });

    Ok((agent, fork_applied))
}

pub struct SpawnAgentHandler {
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
}

#[derive(Deserialize)]
struct SpawnAgentArgs {
    instruction: String,
    role: Option<String>,
    fork_context: Option<bool>,
}

#[async_trait]
impl ToolHandler for SpawnAgentHandler {
    fn name(&self) -> &str {
        "spawn_agent"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Spawn a background agent thread and give it an initial instruction.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "instruction": {
                            "type": "string",
                            "description": "The initial task for the agent."
                        },
                        "role": {
                            "type": "string",
                            "enum": ["default", "explorer", "worker"],
                            "description": "Optional agent role label."
                        },
                        "fork_context": {
                            "type": "boolean",
                            "description": "When true, inherit the parent agent's non-system conversation history."
                        }
                    },
                    "required": ["instruction"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        let args = match serde_json::from_str::<SpawnAgentArgs>(arguments) {
            Ok(args) => args,
            Err(e) => {
                return tool_error(
                    self.name(),
                    "invalid_arguments",
                    format!("failed to parse arguments: {e}"),
                    json!({ "arguments": arguments }),
                )
            }
        };

        let role = match resolve_agent_role(args.role.as_deref()) {
            Ok(role) => role,
            Err(message) => {
                return tool_error(
                    self.name(),
                    "invalid_role",
                    message,
                    json!({ "role": args.role }),
                )
            }
        };
        let fork_context = args.fork_context.unwrap_or(false);
        let (agent, fork_applied) =
            match spawn_delegated_agent(&registry, &context, role, &args.instruction, fork_context)
            {
                Ok(result) => result,
                Err(message) => {
                    return tool_error(
                        self.name(),
                        "spawn_rejected",
                        message,
                        json!({
                            "instruction": args.instruction,
                            "role": role,
                            "fork_context": fork_context,
                        }),
                    )
                }
            };
        println!(
            "[agent-team] spawned agent: {} [{}]",
            agent.id(),
            role.label()
        );

        start_agent_worker(
            Arc::clone(&registry),
            Arc::clone(&agent),
            AgentExecutionConfig {
                api_key: self.api_key.clone(),
                base_url: self.base_url.clone(),
                model_name: self.model_name.clone(),
            },
        );

        tool_success(
            self.name(),
            "agent spawned successfully",
            json!({
                "agent_id": agent.id(),
                "role": role,
                "parent_agent_id": agent.parent_agent_id(),
                "depth": agent.depth(),
                "fork_context_requested": fork_context,
                "fork_context_applied": fork_applied,
                "status": agent.status(),
                "agent_snapshots": registry.agent_snapshots(),
            }),
        )
    }
}

pub struct SendAgentInputHandler {
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
}

#[derive(Deserialize)]
struct SendAgentInputArgs {
    agent_id: String,
    instruction: String,
}

#[async_trait]
impl ToolHandler for SendAgentInputHandler {
    fn name(&self) -> &str {
        "send_input"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Send a follow-up instruction to an existing agent thread.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "The target agent identifier."
                        },
                        "instruction": {
                            "type": "string",
                            "description": "The instruction to queue for the target agent."
                        }
                    },
                    "required": ["agent_id", "instruction"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        _context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        let args = match serde_json::from_str::<SendAgentInputArgs>(arguments) {
            Ok(args) => args,
            Err(e) => {
                return tool_error(
                    self.name(),
                    "invalid_arguments",
                    format!("failed to parse arguments: {e}"),
                    json!({ "arguments": arguments }),
                )
            }
        };

        let Some(agent) = registry.get_agent_thread(&args.agent_id) else {
            return tool_error(
                self.name(),
                "agent_not_found",
                format!("agent {} not found", args.agent_id),
                json!({ "agent_id": args.agent_id }),
            );
        };

        if agent.is_closed() {
            return tool_error(
                self.name(),
                "agent_closed",
                format!("agent {} is closed", args.agent_id),
                json!({ "agent_id": args.agent_id }),
            );
        }

        println!(
            "[agent-team] queued input for {}: {}",
            args.agent_id, args.instruction
        );
        agent.enqueue_input(args.instruction);
        start_agent_worker(
            Arc::clone(&registry),
            Arc::clone(&agent),
            AgentExecutionConfig {
                api_key: self.api_key.clone(),
                base_url: self.base_url.clone(),
                model_name: self.model_name.clone(),
            },
        );

        tool_success(
            self.name(),
            "input queued successfully",
            json!({
                "agent_id": args.agent_id,
                "status": agent.status(),
                "agent_snapshots": registry.agent_snapshots(),
            }),
        )
    }
}

pub struct WaitAgentHandler;

#[derive(Deserialize)]
struct WaitAgentArgs {
    agent_id: String,
    timeout_ms: Option<u64>,
}

#[async_trait]
impl ToolHandler for WaitAgentHandler {
    fn name(&self) -> &str {
        "wait_agent"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Wait for an agent to reach a final state or until the timeout expires.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "The target agent identifier."
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Optional timeout in milliseconds."
                        }
                    },
                    "required": ["agent_id"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        _context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        let args = match serde_json::from_str::<WaitAgentArgs>(arguments) {
            Ok(args) => args,
            Err(e) => {
                return tool_error(
                    self.name(),
                    "invalid_arguments",
                    format!("failed to parse arguments: {e}"),
                    json!({ "arguments": arguments }),
                )
            }
        };

        let Some(agent) = registry.get_agent_thread(&args.agent_id) else {
            return tool_error(
                self.name(),
                "agent_not_found",
                format!("agent {} not found", args.agent_id),
                json!({ "agent_id": args.agent_id }),
            );
        };

        let timeout_duration = Duration::from_millis(
            args.timeout_ms
                .unwrap_or(DEFAULT_WAIT_AGENT_TIMEOUT.as_millis() as u64),
        );
        let (status, timed_out) = wait_for_agent_status(Arc::clone(&agent), timeout_duration).await;

        tool_success(
            self.name(),
            if timed_out {
                "wait timed out"
            } else {
                "agent reached a final state"
            },
            json!({
                "agent_id": args.agent_id,
                "status": status,
                "timed_out": timed_out,
                "final_output": agent.last_result(),
                "error": agent.last_error(),
            }),
        )
    }
}

pub struct CloseAgentHandler;

#[derive(Deserialize)]
struct CloseAgentArgs {
    agent_id: String,
}

#[async_trait]
impl ToolHandler for CloseAgentHandler {
    fn name(&self) -> &str {
        "close_agent"
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Mark an agent as closed so it no longer accepts new work.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "The target agent identifier."
                        }
                    },
                    "required": ["agent_id"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        _context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        let args = match serde_json::from_str::<CloseAgentArgs>(arguments) {
            Ok(args) => args,
            Err(e) => {
                return tool_error(
                    self.name(),
                    "invalid_arguments",
                    format!("failed to parse arguments: {e}"),
                    json!({ "arguments": arguments }),
                )
            }
        };

        let Some(agent) = registry.get_agent_thread(&args.agent_id) else {
            return tool_error(
                self.name(),
                "agent_not_found",
                format!("agent {} not found", args.agent_id),
                json!({ "agent_id": args.agent_id }),
            );
        };

        agent.close();
        println!("[agent-team] closed agent: {}", args.agent_id);

        tool_success(
            self.name(),
            "agent closed successfully",
            json!({
                "agent_id": args.agent_id,
                "status": agent.status(),
                "agent_snapshots": registry.agent_snapshots(),
            }),
        )
    }
}

pub struct ListAgentsHandler;

#[async_trait]
impl ToolHandler for ListAgentsHandler {
    fn name(&self) -> &str {
        "list_agents"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "List the currently known agent threads and their statuses.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        _context: ToolExecutionContext,
        _arguments: &str,
    ) -> String {
        tool_success(
            self.name(),
            "listed agents successfully",
            json!({
                "agents": registry.agent_snapshots(),
            }),
        )
    }
}

// Delegated execution support.

pub struct SubAgentHandler {
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
}

#[derive(Deserialize)]
struct SubAgentArgs {
    instruction: String,
}

#[async_trait]
impl ToolHandler for SubAgentHandler {
    fn name(&self) -> &str {
        "spawn_sub_agent"
    }

    fn requires_dispatch_lock(&self) -> bool {
        false
    }

    fn spec(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": "Spawn a sub-agent to perform a specific sub-task. The sub-agent has access to all your tools.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "instruction": {
                            "type": "string",
                            "description": "The specific instruction for the sub-agent. Be clear and provide necessary context."
                        }
                    },
                    "required": ["instruction"],
                    "additionalProperties": false
                }
            }
        })
    }

    async fn handle(
        &self,
        registry: Arc<ToolRegistry>,
        context: ToolExecutionContext,
        arguments: &str,
    ) -> String {
        let args = match serde_json::from_str::<SubAgentArgs>(arguments) {
            Ok(args) => args,
            Err(e) => {
                return tool_error(
                    self.name(),
                    "invalid_arguments",
                    format!("failed to parse arguments: {e}"),
                    json!({ "arguments": arguments }),
                )
            }
        };

        let (agent, fork_applied) = match spawn_delegated_agent(
            &registry,
            &context,
            AgentRole::Default,
            &args.instruction,
            true,
        ) {
            Ok(result) => result,
            Err(message) => {
                return tool_error(
                    self.name(),
                    "spawn_rejected",
                    message,
                    json!({
                        "instruction": args.instruction,
                    }),
                )
            }
        };
        let agent_snapshot = agent.snapshot();
        println!(
            "[agent-team] spawned agent: {} [{}]",
            agent_snapshot.id,
            agent_snapshot.role.label()
        );
        println!("[sub-agent] task assigned: {}", args.instruction);
        start_agent_worker(
            Arc::clone(&registry),
            Arc::clone(&agent),
            AgentExecutionConfig {
                api_key: self.api_key.clone(),
                base_url: self.base_url.clone(),
                model_name: self.model_name.clone(),
            },
        );

        let (status, timed_out) =
            wait_for_agent_status(Arc::clone(&agent), DEFAULT_WAIT_AGENT_TIMEOUT).await;
        if timed_out {
            return tool_error(
                self.name(),
                "wait_timed_out",
                "sub-agent did not finish before the timeout",
                json!({
                    "agent_id": agent.id(),
                    "instruction": args.instruction.clone(),
                    "status": status,
                }),
            );
        }

        match status {
            AgentStatus::Completed => {
                println!("[sub-agent] task completed");
                tool_success(
                    self.name(),
                    "sub-agent task completed",
                    json!({
                        "agent_id": agent.id(),
                        "instruction": args.instruction.clone(),
                        "fork_context_applied": fork_applied,
                        "final_content": agent.last_result(),
                        "agent_snapshots": registry.agent_snapshots(),
                    }),
                )
            }
            AgentStatus::Failed => tool_error(
                self.name(),
                "sub_agent_failed",
                agent
                    .last_error()
                    .unwrap_or_else(|| "sub-agent failed".to_string()),
                json!({
                    "agent_id": agent.id(),
                    "instruction": args.instruction.clone(),
                    "status": AgentStatus::Failed,
                }),
            ),
            AgentStatus::Closed => tool_error(
                self.name(),
                "sub_agent_closed",
                "sub-agent was closed before completion",
                json!({
                    "agent_id": agent.id(),
                    "instruction": args.instruction.clone(),
                }),
            ),
            AgentStatus::Pending | AgentStatus::Running => tool_error(
                self.name(),
                "sub_agent_incomplete",
                "sub-agent did not reach a final state",
                json!({
                    "agent_id": agent.id(),
                    "instruction": args.instruction.clone(),
                    "status": status,
                }),
            ),
        }
    }
}

// Low-level tool implementations.

fn check_safe_command(cmd: &str) -> Result<(), String> {
    let normalized = cmd.trim().to_ascii_lowercase();
    let dangerous_keywords = [
        "rm -rf",
        "mkfs",
        "dd if=",
        "halt",
        "reboot",
        "shutdown",
        "> /dev/sda",
        "sudo ",
        "chmod -r 777 /",
    ];
    for keyword in dangerous_keywords {
        if normalized.contains(keyword) {
            return Err(format!("command rejected by policy: contains '{keyword}'"));
        }
    }
    Ok(())
}

fn truncate_text(content: &str, max_chars: usize) -> TruncatedText {
    if content.len() <= max_chars {
        return TruncatedText {
            content: content.to_string(),
            truncated: false,
            omitted_chars: 0,
        };
    }

    let half = max_chars / 2;
    let mut prefix_end = half.min(content.len());
    while prefix_end > 0 && !content.is_char_boundary(prefix_end) {
        prefix_end -= 1;
    }

    let mut suffix_start = content.len().saturating_sub(half);
    while suffix_start < content.len() && !content.is_char_boundary(suffix_start) {
        suffix_start += 1;
    }

    let prefix = &content[..prefix_end];
    let suffix = &content[suffix_start..];
    let omitted_chars = content.len().saturating_sub(prefix.len() + suffix.len());

    TruncatedText {
        content: format!("{prefix}\n\n... [TRUNCATED {omitted_chars} CHARACTERS] ...\n\n{suffix}"),
        truncated: true,
        omitted_chars,
    }
}

fn execute_bash(cmd: &str) -> String {
    if let Err(msg) = check_safe_command(cmd) {
        return tool_error("run_bash", "policy_denied", msg, json!({ "cmd": cmd }));
    }

    println!("[sandbox] executing command: {}", cmd);

    let working_directory = match std::env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            return tool_error(
                "run_bash",
                "cwd_unavailable",
                format!("failed to resolve working directory: {e}"),
                json!({ "cmd": cmd }),
            )
        }
    };

    let mut child = match Command::new("bash")
        .arg("-lc")
        .arg(cmd)
        .current_dir(&working_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return tool_error(
                "run_bash",
                "spawn_failed",
                format!("terminal execution failed: {e}"),
                json!({
                    "cmd": cmd,
                    "cwd": working_directory.display().to_string(),
                }),
            )
        }
    };

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                break;
            }
            Ok(None) => {
                if started_at.elapsed() > COMMAND_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return tool_error(
                        "run_bash",
                        "timeout",
                        format!(
                            "command exceeded {} seconds and was terminated",
                            COMMAND_TIMEOUT.as_secs()
                        ),
                        json!({
                            "cmd": cmd,
                            "cwd": working_directory.display().to_string(),
                            "timeout_secs": COMMAND_TIMEOUT.as_secs(),
                        }),
                    );
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return tool_error(
                    "run_bash",
                    "wait_failed",
                    format!("failed while waiting for command completion: {e}"),
                    json!({
                        "cmd": cmd,
                        "cwd": working_directory.display().to_string(),
                    }),
                )
            }
        }
    }

    match child.wait_with_output() {
        Ok(output) => {
            let stdout = truncate_text(
                &String::from_utf8_lossy(&output.stdout),
                BASH_OUTPUT_CHARS_PER_STREAM,
            );
            let stderr = truncate_text(
                &String::from_utf8_lossy(&output.stderr),
                BASH_OUTPUT_CHARS_PER_STREAM,
            );
            let exit_code = output.status.code();
            let succeeded = output.status.success();
            let message = if succeeded {
                "command executed successfully"
            } else {
                "command exited with a non-zero status"
            };

            let data = json!({
                "cmd": cmd,
                "cwd": working_directory.display().to_string(),
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
                "timed_out": false,
            });

            if succeeded {
                tool_success("run_bash", message, data)
            } else {
                tool_error("run_bash", "non_zero_exit", message, data)
            }
        }
        Err(e) => tool_error(
            "run_bash",
            "wait_with_output_failed",
            format!("terminal execution failed after spawn: {e}"),
            json!({
                "cmd": cmd,
                "cwd": working_directory.display().to_string(),
            }),
        ),
    }
}

fn read_file(path: &str) -> String {
    println!("[sandbox] reading file: {}", path);
    match std::fs::read_to_string(path) {
        Ok(content) => tool_success(
            "read_file",
            format!("read file successfully: {path}"),
            json!({
                "path": path,
                "content": truncate_text(&content, MAX_TOOL_CONTENT_CHARS),
            }),
        ),
        Err(e) => tool_error(
            "read_file",
            "read_failed",
            format!("read file failed: {e}"),
            json!({ "path": path }),
        ),
    }
}

fn write_file(path: &str, content: &str) -> String {
    println!("[sandbox] writing file: {}", path);
    match std::fs::write(path, content) {
        Ok(_) => tool_success(
            "write_file",
            format!("file written successfully: {path}"),
            json!({
                "path": path,
                "bytes_written": content.len(),
            }),
        ),
        Err(e) => tool_error(
            "write_file",
            "write_failed",
            format!("write file failed: {e}"),
            json!({
                "path": path,
                "bytes_attempted": content.len(),
            }),
        ),
    }
}

fn edit_file(path: &str, target: &str, replacement: &str) -> String {
    println!("[sandbox] editing file: {}", path);
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if !content.contains(target) {
                return tool_error(
                    "edit_file",
                    "target_not_found",
                    "edit file failed: target string not found",
                    json!({
                        "path": path,
                        "target": target,
                    }),
                );
            }
            let new_content = content.replace(target, replacement);
            match std::fs::write(path, new_content) {
                Ok(_) => tool_success(
                    "edit_file",
                    format!("file edited successfully: {path}"),
                    json!({
                        "path": path,
                        "target": truncate_text(target, MAX_TOOL_CONTENT_CHARS / 4),
                        "replacement": truncate_text(replacement, MAX_TOOL_CONTENT_CHARS / 4),
                    }),
                ),
                Err(e) => tool_error(
                    "edit_file",
                    "write_failed",
                    format!("edit file failed: {e}"),
                    json!({ "path": path }),
                ),
            }
        }
        Err(e) => tool_error(
            "edit_file",
            "read_failed",
            format!("edit file failed: {e}"),
            json!({ "path": path }),
        ),
    }
}
