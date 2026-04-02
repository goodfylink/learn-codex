[中文版](./README.md)

# Step02

The goal of `step02` is to solve a problem that appears quickly after `step01`:

in `step01`, tool-calling logic is mostly hardcoded in the main loop, which makes the example hard to extend.

So the key upgrade in `step02` is not only “more tools”. The main improvement is that tool definition, tool schema generation, and tool execution are separated from the main loop and moved into a minimal tool registry design.

## What this step implements

Relevant source files:

- [step02.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/step02.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/sandbox.rs)

At this stage, the example supports:

- a `ToolRegistry` to manage tools centrally
- a `ToolHandler` trait for each tool implementation
- exporting all tool schemas to the model automatically
- dispatching tool calls through the registry
- multiple basic tools instead of only `run_bash`

The tool set in `step02` is:

- `run_bash`
- `read_file`
- `write_file`
- `edit_file`

So by this point, the agent has evolved from “shell-only” into a basic file-capable agent that can read, write, and edit files.

## What changed compared with Step01

Compared with [step01](/Users/ycyin/code/rust code/demo/src/examples/step01/README.md), `step02` introduces 4 major upgrades:

1. Tool logic is no longer hardcoded in the main loop  
   In `step01`, the main loop directly decides:
   - whether the tool name is `run_bash`
   - how arguments are parsed
   - how the tool is executed

   In `step02`, the flow becomes:
   - tools are registered in `ToolRegistry`
   - the main loop only passes `function_name` and `arguments_json`
   - the specific handler owns the execution

2. The tool count grows from 1 to 4  
   `step01` only has `run_bash`.  
   `step02` adds:
   - `read_file`
   - `write_file`
   - `edit_file`

3. Tool schemas are exported by the registry  
   In `step01`, the tool schema is an inline JSON definition in the main file.  
   In `step02`, each tool provides its own `spec()`, and `registry.get_specs()` aggregates them.

4. The main loop starts to look like a runtime  
   After this step, the main loop mainly handles:
   - conversation history
   - model requests
   - deciding whether tools are needed
   - forwarding tool calls into the registry

   The tool details are no longer embedded in the control loop.

In one sentence:

`step01` proves the agent loop works, while `step02` makes that loop extensible.

## How the main flow changed

The control flow in [step02.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/step02.rs) is still similar to `step01`, but it is cleaner:

1. Initialize the model client and system prompt
2. Create a `ToolRegistry`
3. Register all tool handlers
4. Export `tools_definitions` from the registry
5. Append user input to `conversation_history`
6. Send the model request
7. If the model returns `tool_calls`
   - store the assistant message first
   - process tool calls one by one
   - dispatch them through `registry.dispatch(...)`
   - append the tool results as `role: tool`
8. If there is no tool call
   - print the final response directly

The most important change is:

the main loop no longer knows each tool’s internal implementation. It only knows how to hand off work to the registry.

## What `sandbox.rs` is responsible for

[sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/sandbox.rs) has two responsibilities in this step.

### 1. Defining the tool abstraction layer

This file introduces:

- `ToolHandler`
- `ToolRegistry`

`ToolHandler` requires every tool to provide:

- `name()`
- `spec()`
- `handle(arguments)`

`ToolRegistry` is responsible for:

- registering tools
- storing the tool table
- dispatching execution by tool name
- aggregating all tool schemas

That is the most important engineering improvement in `step02`.

### 2. Implementing four basic tools

The four handlers in this version are:

- `RunBashHandler`
- `ReadFileHandler`
- `WriteFileHandler`
- `EditFileHandler`

They all follow the same pattern:

1. define an argument struct
2. define the tool schema
3. parse the JSON arguments
4. run the actual tool logic
5. return a structured JSON result

This consistent pattern makes later tool expansion much easier.

## Functional focus of this step

`step02` is not only about adding file operations. It also matters for two bigger reasons:

1. Extensibility improves significantly  
   To add another tool later, you only need to:
   - create a handler
   - implement `ToolHandler`
   - register it in the registry

   You no longer need to keep editing branching logic in the main loop.

2. Responsibilities are split more clearly  
   - `step02.rs`: the agent loop
   - `sandbox.rs`: tool abstraction and execution

That layering is much closer to a real runtime structure.

## What this version still does not include

Even though `step02` is much more structured than `step01`, it is still an early-stage example. It does not yet include:

- `update_plan`
- sub-agent delegation
- skill injection
- context compaction
- parallel tool calls
- an agent team

So the role of `step02` is very specific:

it is the “tool system abstraction” step, not a full agent runtime.

## How to run it

This project now registers `step02` as a standalone binary target, so you can run:

```bash
cargo run --bin step02
```

## Example prompts

`step02` adds file-oriented tools on top of `step01`, so prompts can now move beyond pure shell exploration and ask the agent to read or edit files directly.

You can try prompts like:

```text
Read Cargo.toml and tell me what dependencies this project uses.
```

```text
Inspect src/examples/step02/step02.rs and summarize the responsibility of this file.
```

```text
Create a file named notes.txt in the current directory with the content hello from step02.
```

```text
Change hello to hi inside notes.txt.
```

```text
First list the files in the current directory, then read Cargo.toml, and finally summarize what this project is.
```

This version works especially well for:

- reading a specific file
- creating a simple file
- editing file contents
- combining shell and file tools for small tasks

Compared with `step01`, you can now ask more directly for “read this file”, “write this file”, or “edit this file” instead of expressing everything as shell commands.

## One-line summary

Compared with `step01`, the most important improvement in `step02` is the move from hardcoded single-tool logic to a “tool registry + multiple handlers” design, which makes the agent loop extensible.
