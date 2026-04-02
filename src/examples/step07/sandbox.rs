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
    async fn handle(&self, registry: Arc<ToolRegistry>, arguments: &str) -> String;
}

pub struct ToolInvocation {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: String,
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

    pub async fn dispatch(self: Arc<Self>, name: &str, arguments: &str) -> Option<String> {
        let handler = self.handlers.get(name).cloned()?;
        println!("[registry] dispatching tool: {}", name);

        if !handler.requires_dispatch_lock() {
            return Some(handler.handle(self.clone(), arguments).await);
        }

        if handler.supports_parallel_tool_calls() {
            let _guard = self.parallel_execution.read().await;
            Some(handler.handle(self.clone(), arguments).await)
        } else {
            let _guard = self.parallel_execution.write().await;
            Some(handler.handle(self.clone(), arguments).await)
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
                    .dispatch(&invocation.tool_name, &invocation.arguments)
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

    pub fn spawn_agent_thread(
        &self,
        role: &str,
        system_prompt: &str,
        instruction: &str,
    ) -> Arc<AgentThread> {
        self.agent_team
            .spawn_agent(role, system_prompt, instruction)
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

    async fn handle(&self, _registry: Arc<ToolRegistry>, arguments: &str) -> String {
        if let Ok(args) = serde_json::from_str::<RunBashArgs>(arguments) {
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

    async fn handle(&self, _registry: Arc<ToolRegistry>, arguments: &str) -> String {
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

    async fn handle(&self, _registry: Arc<ToolRegistry>, arguments: &str) -> String {
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

    async fn handle(&self, _registry: Arc<ToolRegistry>, arguments: &str) -> String {
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

    async fn handle(&self, registry: Arc<ToolRegistry>, arguments: &str) -> String {
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

    async fn handle(&self, registry: Arc<ToolRegistry>, arguments: &str) -> String {
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

        let system_prompt = "You are a sub-agent assisting a primary agent. You have access to various tools. Perform the task assigned to you efficiently.";
        let agent = registry.spawn_agent_thread("default", system_prompt, &args.instruction);
        let agent_snapshot = agent.snapshot();
        println!(
            "[agent-team] spawned agent: {} [{}]",
            agent_snapshot.id, agent_snapshot.role
        );
        println!("[sub-agent] task assigned: {}", args.instruction);

        let client = Client::new();
        agent.set_status(AgentStatus::Running);

        let tool_specs = registry.get_specs();

        // Nested execution loop for the delegated agent.
        loop {
            let history = agent.history_snapshot();
            let payload = json!({
                "model": self.model_name,
                "messages": history,
                "tools": tool_specs,
                "parallel_tool_calls": DEMO_SUPPORTS_PARALLEL_TOOL_CALLS,
                "temperature": 0.2
            });

            let res = match client
                .post(&self.base_url)
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    agent.set_status(AgentStatus::Failed);
                    return tool_error(
                        self.name(),
                        "sub_agent_request_failed",
                        format!("sub-agent API call failed: {e}"),
                        json!({
                            "agent_id": agent.id(),
                            "instruction": args.instruction.clone(),
                        }),
                    );
                }
            };

            let response_json: serde_json::Value = match res.json().await {
                Ok(json) => json,
                Err(e) => {
                    agent.set_status(AgentStatus::Failed);
                    return tool_error(
                        self.name(),
                        "sub_agent_response_invalid",
                        format!("failed to parse sub-agent response: {e}"),
                        json!({
                            "agent_id": agent.id(),
                            "instruction": args.instruction.clone(),
                        }),
                    );
                }
            };

            let choice = &response_json["choices"][0];
            let message = &choice["message"];

            agent.push_history_item(message.clone());

            if choice["finish_reason"] == "tool_calls" || message["tool_calls"].is_array() {
                let tool_calls = message["tool_calls"].as_array().unwrap();
                println!(
                    "[sub-agent] tool batch received: {} call(s)",
                    tool_calls.len()
                );

                let invocations = tool_calls
                    .iter()
                    .filter_map(|tool_call| {
                        let function_name = tool_call["function"]["name"].as_str()?;
                        let arguments = tool_call["function"]["arguments"].as_str()?;
                        let call_id = tool_call["id"].as_str()?;
                        println!(
                            "[sub-agent] requested tool: {}({})",
                            function_name, arguments
                        );
                        Some(ToolInvocation {
                            call_id: call_id.to_string(),
                            tool_name: function_name.to_string(),
                            arguments: arguments.to_string(),
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
                continue; // 继续循环，让子 Agent 看到工具执行结果
            } else {
                // 子 Agent 完成任务，返回最终文本
                let final_content = message["content"].as_str().unwrap_or("Done").to_string();
                agent.set_status(AgentStatus::Completed);
                println!("[sub-agent] task completed");
                return tool_success(
                    self.name(),
                    "sub-agent task completed",
                    json!({
                        "agent_id": agent.id(),
                        "instruction": args.instruction.clone(),
                        "final_content": final_content,
                        "agent_snapshots": registry.agent_snapshots(),
                    }),
                );
            }
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
