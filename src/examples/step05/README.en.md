[中文版](./README.md)

# Step05

`step05` starts addressing two problems that real runtimes hit very quickly after `step04`:

- conversation history keeps growing
- some tasks need specialized knowledge on demand instead of stuffing everything into the prompt

Relevant source files:

- [step05.rs](/Users/ycyin/code/rust code/demo/src/examples/step05/step05.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step05/sandbox.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step05/skills.rs)

## What this step implements

This version keeps:

- planning
- file tools
- sub-agent delegation

And adds:

- a simplified `compact_history`
- local skill discovery
- rendering a skills section into the system prompt
- turn-level skill injection when the user explicitly mentions a skill

## What changed compared with Step04

Compared with `step04`, the biggest changes in `step05` are:

1. it starts handling long context
2. it introduces skills
3. the system prompt becomes “base instructions + dynamic skills section”

In one sentence:

`step04` solves delegation, while `step05` starts solving “how to preserve context and how to inject knowledge only when needed.”

## Functional focus

### 1. Context compaction

When history becomes too long, `compact_history`:

- keeps the system prompt
- keeps the most recent interaction window
- summarizes older conversation into a compact message

This is a simplified long-context management strategy.

### 2. Skill injection

This step first scans local `skills/`, then renders the list of available skills into the prompt.  
Only when the user explicitly mentions a skill does the runtime inject the relevant skill content into the current turn.

That matches the general direction of the main implementation.

## How to run it

```bash
cargo run --bin step05
```

## Example prompts

This version is useful for testing skill and long-context behavior:

```text
Use $rust-review to review src/main.rs.
```

```text
Read src/examples/step05/step05.rs and explain how skill injection works.
```

```text
I am going to give you a lot of information across multiple messages. Keep the context and continue the task afterward.
```

## One-line summary

The core of `step05` is bringing skills and context compaction into the agent loop, so the system starts to support longer sessions and on-demand knowledge injection.
