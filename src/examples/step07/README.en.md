[中文版](./README.md)

# Step07

`step07` is the first part of the agent-team story: it upgrades an agent from a temporary recursive sub-call into a real managed entity.

Relevant source files:

- [step07.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/step07.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/sandbox.rs)
- [agent_team.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/agent_team.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/skills.rs)

## What this step implements

This version keeps:

- planning
- skills
- context compaction
- parallel tool calls

And adds the first agent entity layer:

- `AgentStatus`
- `AgentSnapshot`
- `AgentThread`
- `AgentTeamManager`

## What changed compared with Step06

Compared with `step06`, the biggest change in `step07` is:

1. a sub-agent is no longer only a temporary recursive execution
2. an agent is now modeled as an independent entity
3. the system starts tracking agent snapshots and status

There is still no full control plane yet, but the first question is now answered:

“what is an agent as an object in the runtime?”

## Functional focus

The most important addition here is not a new tool but a new agent data model:

- an agent has its own `id`
- an agent has its own `role`
- an agent has its own `history`
- an agent has its own `status`

That sets up the later `send_input / wait_agent / close_agent` features.

## How to run it

```bash
cargo run --bin step07
```

## Example prompts

This version is useful for testing whether a sub-agent is now treated like an entity:

```text
Delegate “read src/examples/step07/step07.rs and summarize its responsibility” to a sub-agent.
```

```text
Use a sub-agent to explain the purpose of src/examples/step07/agent_team.rs.
```

```text
Explain what step07 adds to the agent model compared with step06.
```

## One-line summary

The core of `step07` is turning the sub-agent from a temporary execution path into a real agent entity, which is the first step toward an agent team.
