use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::env;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::sync::Arc;

mod sandbox;
#[path = "../../skills.rs"]
mod skills;
use sandbox::EditFileHandler;
use sandbox::PlanHandler;
use sandbox::ReadFileHandler;
use sandbox::RunBashHandler;
use sandbox::SubAgentHandler;
use sandbox::ToolRegistry;
use sandbox::WriteFileHandler;
use skills::build_skill_injection_messages;
use skills::collect_explicit_skill_mentions;
use skills::load_skills;
use skills::render_skills_section;

// Message types used to maintain a function-calling conversation history.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    // 如果大模型想调用工具了，就会给 assistant 追加这个数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    // 如果你是反馈工具执行结果，就要指明这是回答哪一个召唤
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // typically "function"
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // Stringified JSON, 需要我们在收到后自己解出来
}

// Response shapes returned by the chat completion API.

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
    finish_reason: String,
}

async fn compact_history(
    client: &Client,
    api_key: &str,
    base_url: &str,
    model_name: &str,
    history: &mut Vec<Message>,
) -> Result<(), Box<dyn std::error::Error>> {
    let total_chars: usize = history
        .iter()
        .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0))
        .sum();

    // Threshold: 20k characters
    if total_chars < 20000 || history.len() <= 6 {
        return Ok(());
    }

    println!(
        "\n[context] history too long ({} chars); starting compaction",
        total_chars
    );

    // Preserve the system prompt and the most recent interaction window.
    let system_message = history[0].clone();
    let recent_messages = history[history.len() - 5..].to_vec();
    let to_summarize = history[1..history.len() - 5].to_vec();

    let summarization_prompt = "Summarize the following conversation history briefly while preserving key facts, user intents, and important tool outputs. Focus on maintaining context for the next steps.";

    let mut summary_request_history = vec![Message {
        role: "system".to_string(),
        content: Some(summarization_prompt.to_string()),
        tool_calls: None,
        tool_call_id: None,
    }];
    summary_request_history.extend(to_summarize);

    let payload = json!({
        "model": model_name,
        "messages": summary_request_history,
        "temperature": 0.3
    });

    let res = client
        .post(base_url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if res.status().is_success() {
        let response_body: ChatResponse = res.json().await?;
        let summary_text = response_body.choices[0]
            .message
            .content
            .clone()
            .unwrap_or_default();

        println!("[context] compaction completed");

        let summary_message = Message {
            role: "system".to_string(),
            content: Some(format!(
                "Previously in this conversation (Summary):\n{}",
                summary_text
            )),
            tool_calls: None,
            tool_call_id: None,
        };

        let mut new_history = vec![system_message, summary_message];
        new_history.extend(recent_messages);
        *history = new_history;
    } else {
        println!("[context] compaction failed; continuing with full history");
    }

    Ok(())
}

fn compose_system_prompt(base_prompt: &str, skills_section: Option<String>) -> String {
    match skills_section {
        Some(skills_section) => format!("{base_prompt}\n{skills_section}\n"),
        None => base_prompt.to_string(),
    }
}

fn build_request_messages(history: &[Message], turn_skill_messages: &[Message]) -> Vec<Message> {
    if history.is_empty() || turn_skill_messages.is_empty() {
        return history.to_vec();
    }

    let mut request_messages = Vec::with_capacity(history.len() + turn_skill_messages.len());
    request_messages.push(history[0].clone());
    request_messages.extend(turn_skill_messages.iter().cloned());
    request_messages.extend(history[1..].iter().cloned());
    request_messages
}

fn skills_root() -> &'static Path {
    if Path::new("demo/skills").is_dir() {
        Path::new("demo/skills")
    } else {
        Path::new("skills")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY")
        .unwrap_or_else(|_| "your-api_key".to_string());
    let base_url = env::var("OPENAI_BASE_URL").unwrap_or_else(|_| {
        "your-base_url".to_string()
    });
    let model_name = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "your-model".to_string());

    let client = Client::new();

    // Build the tool registry once for the lifetime of the session.
    let mut registry_raw = ToolRegistry::new();
    registry_raw.register(Arc::new(RunBashHandler));
    registry_raw.register(Arc::new(ReadFileHandler));
    registry_raw.register(Arc::new(WriteFileHandler));
    registry_raw.register(Arc::new(EditFileHandler));
    registry_raw.register(Arc::new(PlanHandler));

    // Register delegated execution support.
    registry_raw.register(Arc::new(SubAgentHandler {
        api_key: api_key.clone(),
        base_url: base_url.clone(),
        model_name: model_name.clone(),
    }));

    let registry = Arc::new(registry_raw);

    let base_system_prompt = "
You are a command line assistant. You can execute bash commands and manage files on the user's macOS filesystem.
Always trace out your thinking by using the provided tools to explore the system or execute tasks requested by the user.
Tool results are returned as structured JSON. Inspect the fields before deciding your next action.

For complex tasks, you MUST use the `update_plan` tool to:
1. Decompose the task into several manageable steps at the beginning.
2. Mark steps as `in_progress` when you start them.
3. Mark steps as `completed` when they are done.

You also have a `spawn_sub_agent` tool. You can use it to delegate specific sub-tasks to a separate agent instance.
";

    let mut conversation_history = vec![Message {
        role: "system".to_string(),
        content: Some(base_system_prompt.to_string()),
        tool_calls: None,
        tool_call_id: None,
    }];

    // Export the tool schema that will be sent to the model.
    let tools_definitions = registry.get_specs();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        let user_input = user_input.trim();

        if user_input.is_empty() {
            continue;
        }
        if user_input == "exit" || user_input == "quit" {
            println!("Bye!");
            break;
        }

        let loaded_skills = load_skills(skills_root());
        for warning in &loaded_skills.warnings {
            println!("[skills] {}", warning);
        }
        let system_prompt = compose_system_prompt(
            base_system_prompt,
            render_skills_section(&loaded_skills.skills),
        );
        conversation_history[0].content = Some(system_prompt);

        let mentioned_skills = collect_explicit_skill_mentions(user_input, &loaded_skills.skills);
        let (skill_messages, skill_warnings) = build_skill_injection_messages(&mentioned_skills);
        for warning in skill_warnings {
            println!("[skills] {}", warning);
        }
        for skill in &mentioned_skills {
            println!("[skills] injecting skill for turn: {}", skill.name);
        }
        let turn_skill_messages = skill_messages
            .into_iter()
            .map(|contents| Message {
                role: "system".to_string(),
                content: Some(contents),
                tool_calls: None,
                tool_call_id: None,
            })
            .collect::<Vec<_>>();

        conversation_history.push(Message {
            role: "user".to_string(),
            content: Some(user_input.into()),
            tool_calls: None,
            tool_call_id: None,
        });

        loop {
            // Compact history before the next sampling request when needed.
            if let Err(e) = compact_history(
                &client,
                &api_key,
                &base_url,
                &model_name,
                &mut conversation_history,
            )
            .await
            {
                println!("[context] compaction error: {}", e);
            }

            let request_messages =
                build_request_messages(&conversation_history, &turn_skill_messages);

            let payload = json!({
                "model": model_name,
                "messages": request_messages,
                "tools": tools_definitions,
                "temperature": 0.2
            });

            let res = client
                .post(&base_url)
                .bearer_auth(&api_key)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await?;

            if !res.status().is_success() {
                let err_text = res.text().await?;
                println!("\nAPI Error: {}", err_text);
                break;
            }

            let response_body: ChatResponse = res.json().await?;
            let choice = &response_body.choices[0];
            let assistant_message = choice.message.clone();

            conversation_history.push(assistant_message.clone());

            if choice.finish_reason == "tool_calls" || assistant_message.tool_calls.is_some() {
                let tool_calls = assistant_message.tool_calls.unwrap();

                for tool_call in tool_calls {
                    let function_name = &tool_call.function.name;
                    let arguments_json = &tool_call.function.arguments;

                    println!(
                        "\n[registry] tool request: {}({})",
                        function_name, arguments_json
                    );

                    // Clone the registry handle so delegated tools can re-enter the dispatcher.
                    if let Some(tool_output) = Arc::clone(&registry)
                        .dispatch(function_name, arguments_json)
                        .await
                    {
                        conversation_history.push(Message {
                            role: "tool".to_string(),
                            content: Some(tool_output),
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_calls: None,
                        });
                    } else {
                        println!("[registry] tool not found: {}", function_name);
                        conversation_history.push(Message {
                            role: "tool".to_string(),
                            content: Some(
                                json!({
                                    "ok": false,
                                    "tool": function_name,
                                    "message": format!("tool {function_name} not found"),
                                    "error_code": "tool_not_found",
                                    "data": {
                                        "arguments": arguments_json,
                                    }
                                })
                                .to_string(),
                            ),
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_calls: None,
                        });
                    }
                }
                continue;
            } else {
                if let Some(text) = assistant_message.content {
                    println!("\n[agent] final response\n{}\n", text);
                }
                break;
            }
        }
    }

    Ok(())
}
