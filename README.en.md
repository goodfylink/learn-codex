# Agent Harness Primer

[中文版](./README.md)

> **The model makes decisions; the harness turns decisions into outcomes.**
>

---

## Overview

Based on OpenAI's public positioning, Codex is best understood as an **AI coding agent**, not just a chat model that can write code. The emphasis is usually on capabilities like:

- collaborating with developers inside local tools
- delegating tasks to background environments
- editing code, running commands, and executing tests in isolated environments
- handling multiple tasks in parallel when needed

So if we want to understand Codex, the focus is not only the model itself, but this combined system:

```text
Model + Tools + Context + State + Parallelism + Permissions + Feedback
```

That is why this project is not primarily about "writing a better prompt." It is about incrementally building a Codex-like harness/runtime.

Over the last two years, a lot of terms have become popular around large models:

- prompt engineering
- context engineering
- agent engineering
- harness engineering
- eval-driven development

From an engineering perspective, these terms form a fairly clear progression:

```text
Prompt -> Context -> Tools -> Runtime -> Harness -> Evals
```

This is not just buzzword stacking. Each layer emerged because a new class of problems became visible:

- At first, people noticed that prompt phrasing strongly affected results.
- Then they realized failures often came from missing context, not weak reasoning.
- Next, they found that one-shot generation was not enough; models needed tools, feedback, and iteration.
- After that, the real challenges became state, permissions, isolation, recovery, parallelism, and evaluation.

The core message of this README is simple:

**Once the model is strong enough, engineering leverage increasingly comes from harness quality.**

---

## Core Ideas

### Model

The model is the underlying capability.

It is responsible for:

- understanding input
- forming judgments
- choosing the next action
- deciding whether to call a tool
- deciding when to stop or continue

From an engineering perspective, the model is closer to a **decision engine**.

The model matters a lot, but it is not the whole system. A strong model can still perform poorly in real tasks if it lacks the right context, tools, state handling, and permission boundaries.

### Prompting

A prompt is how you express instructions to the model.

Prompt engineering focuses on:

- making goals explicit
- reducing ambiguity
- constraining output format
- improving stability

It solves part of the input problem, but not the full systems problem.

### Context Engineering

Context is the actual set of information the model sees at a given moment.

Context engineering focuses on:

- what information should be shown
- what should be excluded
- what should be loaded on demand
- what history should be compacted
- what outputs must remain lossless
- what noise must be isolated

Prompting is closer to "how you say it." Context engineering is closer to "what the model really sees."

### Agent Loop

An agent is not just a single API call, and not just a prompt template.

A minimal agent needs at least:

- a model
- tools
- a loop
- feedback
- state

It can act repeatedly around a goal instead of producing only one response.

### Harness Layer

The harness is the engineering layer that lets an agent work in a real environment.

It can be summarized like this:

```text
Harness = Tools + Context + State + Memory + Permissions + Feedback
```

That usually includes:

- **Tools**: files, shell, databases, browsers, APIs
- **Context**: task info, codebase state, docs, previous actions
- **State**: todo items, dependencies, background tasks, checkpoints
- **Memory**: summaries, long-term knowledge, persisted goals
- **Permissions**: sandboxing, approvals, network boundaries, trust zones
- **Feedback**: logs, errors, diffs, execution outputs, evaluation signals

The harness does not think for the model. The harness gives the model a world it can work in.

### Evals

Evals are the mechanisms used to assess system behavior.

They answer questions like:

- did the system actually improve?
- did a new prompt, model, or tool cause regressions?
- did failure happen at the result layer or the process layer?
- did one change break another scenario?

Without evals, AI systems struggle to form a real engineering feedback loop.

---

## Minimal Architecture

No matter what naming is used, many agent systems reduce to a loop like this:

```python
def agent_loop(messages):
    while True:
        response = model(messages=messages, tools=TOOLS)
        messages.append(response)

        if response.stop_reason != "tool_use":
            return response

        results = []
        for tool_call in response.tool_calls:
            output = TOOL_HANDLERS[tool_call.name](**tool_call.input)
            results.append({
                "type": "tool_result",
                "tool_use_id": tool_call.id,
                "content": output,
            })

        messages.append({"role": "user", "content": results})
```

This loop is small, but it is already enough to create agent behavior:

- the model decides the next step
- the system executes tools
- tool results go back into context
- the cycle continues until completion

The real engineering complexity is not the loop itself. It is the entire layer wrapped around the loop.

---

## Why Prompting Is Not Enough

Prompt engineering became popular first because it exposed the most obvious issue:

**The same model can behave very differently depending on instruction wording.**

But once systems enter real environments, failures usually start to come from other places:

- the model did not see key context
- the tool interfaces were too coarse
- history became overloaded
- state was not persisted
- permission boundaries were unclear
- failure recovery was missing
- regressions could not be measured

At that point, focusing only on prompts becomes insufficient.

That is why the industry naturally moves from Prompt to Context, and then from Context to Harness.

---

## Why Context Matters But Is Not the Whole System

Context engineering is extremely important right now.

Many agent failures do not come from weak reasoning, but from issues like:

- the wrong information was shown
- too much information was shown
- information was shown in a confusing order
- important history was buried under noise
- tool outputs were not usable enough for the model

But context still answers only one question:

**What does the model see right now?**

A real system also needs to answer:

- what can the model do right now?
- where does execution happen?
- how are actions recorded and recovered?
- what requires approval?
- what can run in parallel and what must stay isolated?
- how do we know the system is actually improving?

Taken together, those questions belong to the harness layer.

---

## Why Systems Like Codex CLI and Claude Code Matter

These systems matter not because they are "better chatbots," but because they show something important:

**Once the model is good enough, the quality gap mostly comes from harness design.**

For a coding agent, a genuinely useful system usually includes more than a single answer:

- reading and editing files
- running commands
- collecting errors
- feeding results back
- loading project-level configuration
- loading skills or documentation
- approval modes
- sandbox boundaries
- sustained task progression

So systems like Codex CLI and Claude Code are better understood as:

**harness systems made of an agent runtime + tools + state + permissions + feedback**

rather than just prompt or context extensions.

### How This Project Maps To That Evolution

The purpose of this project is to split that evolution into concrete, understandable steps.

The current main implementation lives in:

- `src/main.rs`
- `src/sandbox.rs`
- `src/skills.rs`
- `src/agent_team.rs`

And `src/examples` explains how each capability grows step by step.

#### Step 01: Minimal Agent Loop

Files:

- `src/examples/step01/step01.rs`
- `src/examples/step01/sandbox.rs`

What it adds:

- the minimal function-calling loop
- a single tool: `run_bash`
- tool results written back into history before the next model turn

This stage corresponds to:

```text
Prompt + Tools + Loop
```

#### Step 02: Tool Registry

Files:

- `src/examples/step02/step02.rs`
- `src/examples/step02/sandbox.rs`

What it adds:

- `ToolRegistry`
- tools extracted out of the main loop
- registered handlers for `run_bash`, `read_file`, `write_file`, and `edit_file`

This is where a key harness property appears:

**Tools stop being `if/else` branches and become managed runtime capabilities.**

#### Step 03: Planning

Files:

- `src/examples/step03/step03.rs`
- `src/examples/step03/sandbox.rs`

What it adds:

- `update_plan`
- explicit task decomposition and step state updates

This stage moves from "can use tools" to "can organize work."

#### Step 04: Delegation

Files:

- `src/examples/step04/step04.rs`
- `src/examples/step04/sandbox.rs`

What it adds:

- `spawn_sub_agent`
- the first ability to delegate a sub-task to another agent loop

At this point the sub-agent is still lightweight and recursive, but it already shows an important idea:

**An agent runtime needs delegation, not just single-threaded execution.**

#### Step 05: Skills And Context Compaction

Files:

- `src/examples/step05/step05.rs`
- `src/examples/step05/sandbox.rs`
- `src/examples/step05/skills.rs`

What it adds:

- `compact_history(...)`
- local skill discovery
- rendering the skill index into the prompt
- injecting `SKILL.md` on a per-turn basis when explicitly mentioned

This stage solves two classic harness problems:

- how to sustain long conversations
- how to load external knowledge on demand

#### Step 06: Parallel Tool Calls

Files:

- `src/examples/step06/step06.rs`
- `src/examples/step06/sandbox.rs`
- `src/examples/step06/skills.rs`

What it adds:

- `parallel_tool_calls`
- batch execution of multiple tool calls in one turn
- parallel dispatch for read-oriented and shell-oriented work

This is where the project starts to align with a very important Codex trait:

**A real coding agent does not serialize every action.**

#### Step 07: Agent Team Part 1

Files:

- `src/examples/step07/step07.rs`
- `src/examples/step07/sandbox.rs`
- `src/examples/step07/skills.rs`
- `src/examples/step07/agent_team.rs`

What it adds:

- `AgentThread`
- `AgentTeamManager`
- agent identity as a managed runtime entity

This stage answers:

**What is an agent as a runtime object?**

#### Step 08: Agent Team Part 2

Files:

- `src/examples/step08/step08.rs`
- `src/examples/step08/sandbox.rs`
- `src/examples/step08/skills.rs`
- `src/examples/step08/agent_team.rs`

What it adds:

- `spawn_agent`
- `send_input`
- `wait_agent`
- `close_agent`
- `list_agents`
- input queues
- status subscriptions
- background workers

At this stage, an agent is no longer just an object. It becomes a reusable worker that can collaborate over time.

#### Step 09: Agent Team Part 3

Files:

- `src/examples/step09/step09.rs`
- `src/examples/step09/sandbox.rs`
- `src/examples/step09/skills.rs`
- `src/examples/step09/agent_team.rs`

What it adds:

- `AgentRole`
- `ToolExecutionContext`
- `parent_agent_id`
- `depth`
- `fork_context`
- minimal explorer-role permission boundaries
- automatic child-to-parent result writeback

This is where the project gets meaningfully close to Codex team semantics:

**A team is not just multiple agents. It needs roles, hierarchy, inherited context, and result reporting.**

### What `src/main.rs` Represents

The current `src/main.rs` is best understood as:

```text
the merged mainline built from step05 + step06 + step07 + step08 + step09
```

It already includes:

- the tool-calling loop
- structured tool results
- skill injection
- context compaction
- parallel tool execution
- sub-agent delegation
- all three layers of agent team capability

So if you want to learn the evolution, start with `examples`.

If you want to inspect the current end-to-end implementation, start with:

- `src/main.rs`
- `src/sandbox.rs`
- `src/agent_team.rs`

---

## What Harness Engineers Actually Build

From this perspective, harness engineers are mostly building the layers below.

### Tooling

They give the model action surfaces such as:

- reading and writing files
- running commands
- searching code
- calling databases
- calling browsers
- calling business APIs

Good tools usually have three properties:

- **atomic**: one clear action at a time
- **clear**: the model knows when to use them and what goes in and out
- **composable**: multiple tools can be chained into larger workflows

### Context Management

They provide high-quality model inputs:

- what the current task needs
- what should be loaded on demand
- what history should be compacted
- what outputs must stay lossless
- what noise must be isolated

### State And Memory

They let the system keep working over time:

- todos
- dependency relationships
- background tasks
- long-term goals
- recovery points
- sub-task results

### Permissions And Safety

They let the system automate without losing control:

- which directories are writable
- which commands need approval
- whether network access is allowed
- whether production resources are reachable
- which operations must stay human-confirmed

### Feedback And Evaluation

They make the system improvable:

- logs
- errors
- diffs
- cost
- latency
- success rate
- regression coverage

In other words, harness engineers are not handwriting intelligence. They are building the working environment for intelligence.

---

## Why Evals Cannot Be An Afterthought

Once a system supports:

- many tools
- long context
- multi-turn action
- parallelism
- sub-agents
- team coordination

then even a small change can cause regressions.

For example:

- a prompt change may weaken planning
- a tool schema change may break old calls
- a compaction change may drop critical history
- a sub-agent permission change may block task completion

That is why evals are not something to bolt on at the end. They are part of what makes a harness maintainable.

---

## How To Read This Project

Recommended order:

1. `src/examples/step01/step01.rs`
2. `src/examples/step02/step02.rs`
3. `src/examples/step03/step03.rs`
4. `src/examples/step04/step04.rs`
5. `src/examples/step05/step05.rs`
6. `src/examples/step06/step06.rs`
7. `src/examples/step07/agent_team.rs`
8. `src/examples/step08/agent_team.rs`
9. `src/examples/step09/agent_team.rs`
10. `src/main.rs`
11. `src/sandbox.rs`

You can also compress the project into three stages:

- basic agent runtime: `step01` to `step04`
- runtime quality improvements: `step05` to `step06`
- agent team: `step07` to `step09`

---

## How To Run

Run from the project root:

```bash
cargo run
```

If you are in the parent directory, you can also point Cargo at the manifest explicitly:

```bash
cargo run --manifest-path ./Cargo.toml
```

Common environment variables:

```bash
export OPENAI_API_KEY=your_key
export OPENAI_BASE_URL=your_endpoint
export OPENAI_MODEL_NAME=your_model
```

Exit with:

```text
exit
```

Example prompts:

```text
Read src/main.rs and summarize what the main loop does
```

```text
Use $rust-review to review src/sandbox.rs
```

```text
Create an explorer agent with fork_context=true and ask it to analyze the tool scheduling logic in src/sandbox.rs
```

```text
Then create a worker agent and ask it to continue based on the explorer's findings
```

---

## Current Limitations

This project is still a teaching implementation, not a production runtime.

Current simplifications include:

- context compaction is heuristic rather than token-accurate
- safety policy remains teaching-grade
- `fork_context` is a simplified history inheritance model
- `close_agent` does not hard-cancel an in-flight HTTP request
- role boundaries are only minimally implemented
- there is no full event system, recovery layer, or evaluation framework

But for understanding the core structure of a Codex-style harness, this implementation is already enough.
