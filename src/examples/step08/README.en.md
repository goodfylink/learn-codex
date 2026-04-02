[中文版](./README.md)

# Step08

`step08` is the second part of the agent-team story: the agent entities do not only exist anymore, they can now actually collaborate.

Relevant source files:

- [step08.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/step08.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/sandbox.rs)
- [agent_team.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/agent_team.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/skills.rs)

## What this step implements

This version adds the minimal agent control plane:

- `spawn_agent`
- `send_input`
- `wait_agent`
- `close_agent`
- `list_agents`

The agent entity itself is also upgraded with:

- an input queue
- status subscription
- final result / error tracking
- a background worker

## What changed compared with Step07

Compared with `step07`, the biggest change in `step08` is:

1. an agent is no longer only an object with `id` and `status`
2. an agent can receive follow-up inputs
3. an agent can be waited on, closed, and listed

In other words, this step answers the second agent-team question:

“how do these agents actually collaborate?”

## Functional focus

This version is already close to a minimal multi-agent control plane:

- create a background agent
- send more work to that agent
- wait for it to complete
- inspect all known agents
- close a specific agent

So this is where the runtime starts to feel like a real multi-agent system.

## How to run it

```bash
cargo run --bin step08
```

## Example prompts

This version is best for testing the full collaboration loop:

```text
Create an agent and let it read src/examples/step08/step08.rs, then summarize the responsibility of this file.
```

```text
List the current agents, then send a follow-up to the same agent and ask it to continue analyzing sandbox.rs.
```

```text
Wait for that agent to finish, but return the current status if it does not complete within 5 seconds.
```

```text
Close that agent.
```

## One-line summary

The core of `step08` is turning agent entities into a minimal collaboration control plane that can create, reuse, wait for, and close agents.
