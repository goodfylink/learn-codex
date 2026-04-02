[中文版](./README.md)

# Step04

`step04` adds the first delegation capability on top of `step03`: the agent can hand off a bounded subtask to a sub-agent.

Relevant source files:

- [step04.rs](/Users/ycyin/code/rust code/demo/src/examples/step04/step04.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step04/sandbox.rs)

## What this step implements

This version keeps:

- the tool registry
- the file tools
- `update_plan`

And adds:

- `spawn_sub_agent`
- a minimal execution loop for the sub-agent
- the ability for the main agent to delegate a bounded task

## What changed compared with Step03

Compared with `step03`, the biggest change in `step04` is:

1. there is no longer only one agent doing everything
2. `SubAgentHandler` is introduced
3. the main agent can hand a specific subtask to another agent loop

This sub-agent is still intentionally lightweight:

- it is closer to a recursive mini agent loop
- it is not yet a persistent agent entity
- there is still no `send_input / wait_agent / close_agent`

So the point of this step is to understand delegation itself.

## Functional focus

The most important new idea in `step04` is a second execution strategy:

- if the task is simple, the main agent handles it directly
- if the task is independent enough, it can use `spawn_sub_agent`

That becomes the foundation for the later agent-team steps.

## How to run it

```bash
cargo run --bin step04
```

## Example prompts

This version is best for testing delegation:

```text
Delegate “read src/examples/step04/step04.rs and summarize its responsibility” to a sub-agent.
```

```text
First make a plan, then let a sub-agent inspect the dependencies in Cargo.toml.
```

```text
Use a sub-agent to analyze what was added in src/examples/step04/sandbox.rs.
```

## One-line summary

The core upgrade in `step04` is that the agent can now delegate a subtask to another agent loop for the first time.
