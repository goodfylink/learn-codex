use serde::Serialize;
use serde_json::json;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const BASH_OUTPUT_CHARS_PER_STREAM: usize = 4_000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

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

/// Apply a simple policy check before launching a shell command.
pub fn check_safe_command(cmd: &str) -> Result<(), String> {
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

/// Execute a shell command and return a structured tool result.
pub fn execute_bash(cmd: &str) -> String {
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
            Ok(Some(_status)) => break,
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
            let data = json!({
                "cmd": cmd,
                "cwd": working_directory.display().to_string(),
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
                "timed_out": false,
            });

            if output.status.success() {
                tool_success("run_bash", "command executed successfully", data)
            } else {
                tool_error(
                    "run_bash",
                    "non_zero_exit",
                    "command exited with a non-zero status",
                    data,
                )
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
