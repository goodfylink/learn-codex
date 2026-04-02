[中文版](./README.md)

# Step03

`step03` builds on `step02` by adding a planning layer. The agent is no longer only executing tools; it can now break down a complex task into steps and update progress explicitly.

Relevant source files:

- [step03.rs](/Users/ycyin/code/rust code/demo/src/examples/step03/step03.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step03/sandbox.rs)

## What this step implements

This version keeps the tool registry and file tools from `step02`, and adds:

- the `update_plan` tool
- a stronger system prompt policy
- a basic “plan first, execute second” workflow

Current tools:

- `run_bash`
- `read_file`
- `write_file`
- `edit_file`
- `update_plan`

## What changed compared with Step02

Compared with `step02`, the key upgrade in `step03` is:

1. the runtime now cares about task organization, not only execution
2. `PlanHandler` makes planning an explicit tool call
3. the system prompt requires complex tasks to be decomposed and tracked with `in_progress` and `completed`

In one sentence:

`step02` makes the tool system extensible, while `step03` gives the agent a minimal orchestration workflow.

## Functional focus

The main point of this step is not many new low-level abilities. It is a more stable working pattern:

1. update the plan
2. start execution
3. keep updating progress while working

That makes later delegation, parallelism, and multi-agent behavior much easier to add.

## How to run it

```bash
cargo run --bin step03
```

## Example prompts

This step is best for testing “plan first, act second” behavior:

```text
Please make a plan first, then analyze the structure of this project.
```

```text
Break this into steps: inspect Cargo.toml, read the step03 code, and summarize the responsibility of this step.
```

```text
First create a plan, then read src/examples/step03/step03.rs and explain what it adds compared with step02.
```

## One-line summary

The core of `step03` is integrating `update_plan` into the agent loop, so the agent evolves from “can use tools” to “can plan before executing”.
