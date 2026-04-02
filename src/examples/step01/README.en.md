[中文版](./README.md)

# Step01

`step01` is the starting point of this project. It implements the smallest useful agent loop:

- accept user input
- call the model
- let the model decide whether to call a tool
- execute the tool
- append the tool result back into context
- call the model again to continue the response

The goal of this step is not full engineering quality. It is to make the core control flow of a function-calling agent visible and easy to understand.

## What this step implements

`step01` only exposes one tool: `run_bash`.

Relevant source files:

- [step01.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/step01.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/sandbox.rs)

At this stage, the example already supports:

- maintaining a minimal `conversation_history`
- sending the tool schema to the model
- reading `tool_calls` from the model response
- parsing `run_bash` arguments
- executing a shell command
- appending the tool result as a `role: tool` message
- asking the model again so it can continue reasoning from the tool output

That is the base layer for every later step in the project.

## How the main loop works

The main loop in [step01.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/step01.rs) can be summarized like this:

1. Read environment variables  
   Mainly:
   - `OPENAI_API_KEY`
   - `OPENAI_BASE_URL`
   - `OPENAI_MODEL_NAME`

2. Initialize the system prompt and conversation history  
   The system prompt tells the model:
   - `run_bash` is available
   - it should call the tool before answering when needed

3. Define a single tool schema  
   There is no registry yet. The `run_bash` schema is written inline in the main file.

4. Read user input and append it to `conversation_history`

5. Send a chat completion request  
   The payload includes:
   - `model`
   - `messages`
   - `tools`
   - `temperature`

6. Process the model response  
   If the model returns `tool_calls`:
   - append the assistant message first
   - take the first tool call
   - parse its arguments
   - execute `run_bash`
   - append the tool output as a `role: tool` message
   - continue the loop

7. If the model does not call a tool  
   Print the final text response directly

This is the smallest ReAct / function-calling agent loop in the project.

## What `sandbox.rs` is responsible for

[sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/sandbox.rs) contains the execution logic for `run_bash`.

It handles 4 important concerns:

1. Risk filtering  
   It blocks obviously dangerous commands such as:
   - `rm -rf`
   - `mkfs`
   - `dd if=`
   - `shutdown`
   - `sudo`

2. Timeout control  
   Commands that run longer than 10 seconds are terminated.

3. Output truncation  
   Long `stdout` and `stderr` are truncated so tool output does not overwhelm the context window.

4. Structured tool output  
   The tool returns a structured JSON string with fields such as:
   - `ok`
   - `tool`
   - `message`
   - `error_code`
   - `data`

This makes the result more stable for the model to consume and easier to extend later.

## Why this step matters

The most important value of `step01` is not the number of tools. It is that it makes this protocol work end to end:

- the assistant emits a tool call
- the local runtime executes the tool
- a `role: tool` message is appended
- the model continues reasoning from that observation

Once that loop exists, later steps can build on it with:

- a tool registry
- planning
- sub-agents
- skills
- context compaction
- parallel tool calls
- an agent team

## Limitations of this version

As the first step, it is intentionally limited:

- only one tool: `run_bash`
- only the first tool call is handled
- tool dispatch is hardcoded
- no `update_plan`
- no skill injection
- no context compaction
- no parallel tool execution
- no agent or sub-agent control plane

So this step is best used to understand how the minimal agent loop works, not as a full runtime.

## How to run it

If the project root is `/Users/ycyin/code/rust code/demo`, you can run:

```bash
cargo run --bin step01
```

If this example is not currently configured as a standalone binary target, treat it as a source-reading example and focus on:

- [step01.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/step01.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/sandbox.rs)

## Example prompts

`step01` only has the `run_bash` tool, so the best prompts are simple requests that can be solved through shell commands.

You can try prompts like:

```text
Show me the current working directory.
```

```text
List the files in the current directory.
```

```text
Read the contents of Cargo.toml.
```

```text
Run pwd and ls -la, then tell me what you found.
```

This version works best for:

- checking the current directory
- listing files
- reading file contents
- running simple shell commands

## One-line summary

`step01` does one thing clearly: it uses a single `run_bash` tool to make the smallest function-calling agent loop work end to end, which becomes the foundation for every later step.
