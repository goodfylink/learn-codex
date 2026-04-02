[中文版](./README.md)

# Step06

`step06` starts aligning with a very important Codex behavior: multiple independent tool calls in the same round can run in parallel instead of strictly one after another.

Relevant source files:

- [step06.rs](/Users/ycyin/code/rust code/demo/src/examples/step06/step06.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step06/sandbox.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step06/skills.rs)

## What this step implements

This version keeps:

- planning
- sub-agents
- skills
- context compaction

And adds:

- `parallel_tool_calls`
- `ToolInvocation`
- `dispatch_many`
- batch execution for tools that are safe to run in parallel

## What changed compared with Step05

Compared with `step05`, the key upgrade in `step06` is:

1. one round can now receive a batch of tool calls
2. the registry can dispatch a batch
3. tools can declare whether they support parallel execution

So this step solves:

“the agent can do more than call multiple tools; it can call multiple independent tools more efficiently in the same round.”

## Functional focus

In this version:

- `run_bash`
- `read_file`

are treated as better candidates for parallel execution, while write-oriented tools remain more serialized.

That makes the runtime much closer to a real tool scheduling layer.

## How to run it

```bash
cargo run --bin step06
```

## Example prompts

This version is best for testing concurrent reads and checks:

```text
Read Cargo.toml and src/main.rs in parallel, then summarize the results.
```

```text
Inspect src/examples/step05/step05.rs and src/examples/step06/step06.rs at the same time, then explain what step06 adds.
```

```text
Run pwd and read Cargo.toml in parallel, then tell me the project structure.
```

## One-line summary

The core of `step06` is upgrading the agent loop from sequential tool execution to batched parallel tool calls within a single round.
