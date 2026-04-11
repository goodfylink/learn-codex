use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::env;
use std::io::Write;
use std::io::{self};

mod sandbox;

// Message types used to maintain a function-calling conversation history.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    // Tool calls are attached to assistant messages when the model requests execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    // Tool responses reference the originating tool call by id.
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

#[derive(Deserialize, Debug)]
struct RunBashArgs {
    cmd: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| "your-api_key".to_string());
    let base_url = env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "your-base_url".to_string());
    let model_name = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "your-model".to_string());

    let client = Client::new();

    let system_prompt = "
You are a command line assistant. You can execute bash commands on the user's macOS filesystem.
Always trace out your thinking by using the `run_bash` tool to explore the system or execute tasks requested by the user.
Tool results are returned as structured JSON. Inspect the fields before deciding your next action.
";

    let mut conversation_history = vec![Message {
        role: "system".to_string(),
        content: Some(system_prompt.to_string()),
        tool_calls: None,
        tool_call_id: None,
    }];

    // Describe the available tool using the function-calling schema.
    let tools_definitions = json!([
        {
            "type": "function",
            "function": {
                "name": "run_bash",
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
        }
    ]);

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

        conversation_history.push(Message {
            role: "user".to_string(),
            content: Some(user_input.into()),
            tool_calls: None,
            tool_call_id: None,
        });
        loop {
            let payload = json!({
                "model": model_name,
                "messages": conversation_history,
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

            // Preserve the full assistant message to keep the protocol history consistent.
            conversation_history.push(assistant_message.clone());

            // Decide whether the model is requesting tool execution or returning a final answer.
            if choice.finish_reason == "tool_calls" || assistant_message.tool_calls.is_some() {
                let tool_calls = assistant_message.tool_calls.unwrap();

                // This step keeps the example minimal by handling only the first tool call.
                if let Some(tool_call) = tool_calls.first() {
                    let function_name = &tool_call.function.name;
                    let arguments_json = &tool_call.function.arguments;

                    println!(
                        "\n[agent] requested tool: {}({})",
                        function_name, arguments_json
                    );

                    if function_name == "run_bash" {
                        // Parse the JSON-encoded arguments produced by the model.
                        if let Ok(args) = serde_json::from_str::<RunBashArgs>(arguments_json) {
                            // Execute the requested command.
                            let tool_output = sandbox::execute_bash(&args.cmd);

                            // Attach the tool result using the native protocol shape.
                            conversation_history.push(Message {
                                role: "tool".to_string(),
                                content: Some(tool_output),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls: None,
                            });

                            // Continue the loop so the model can observe the tool result.
                            continue;
                        } else {
                            println!("Failed to parse bash arguments: {}", arguments_json);
                        }
                    }
                }
            } else {
                // No additional tool call means the turn is complete.
                if let Some(text) = assistant_message.content {
                    println!("\n[agent] final response\n{}\n", text);
                }
                break;
            }
        }
    }

    Ok(())
}
