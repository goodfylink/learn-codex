[中文版](./README.md)

# Step09

`step09` is the third part of the agent-team story: it turns a set of collaborating agents into an organized team.

Relevant source files:

- [step09.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/step09.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/sandbox.rs)
- [agent_team.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/agent_team.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/skills.rs)

## What this step implements

On top of the `step08` control plane, this version adds team organization:

- `AgentRole`
- `parent_agent_id`
- `depth`
- `fork_context`
- parent/child agent coordination
- distinct `explorer / worker / default` roles
- child-agent result propagation back to the parent

## What changed compared with Step08

Compared with `step08`, the biggest changes in `step09` are:

1. agents now have formal roles instead of only string labels
2. agents now have parent-child relationships
3. a child agent can start with the parent’s context when needed
4. child-agent completion or failure can be written back to the parent agent

In one sentence:

`step08` solves “how agents collaborate,” while `step09` solves “how an agent team is organized.”

## Functional focus

### 1. Roles

This version supports:

- `default`
- `explorer`
- `worker`

`explorer` already has a minimal permission boundary and is better suited for read-only analysis work.

### 2. `fork_context`

When creating a child agent, the runtime can decide whether the child inherits context from the parent.  
That allows the child to start with useful background when needed.

### 3. Parent-child feedback

When a child agent completes, fails, or closes, the parent can receive an update.  
This is what turns the system from a loose set of agents into a team with a feedback loop.

## How to run it

```bash
cargo run --bin step09
```

## Example prompts

This version is best for testing roles, `fork_context`, and team coordination:

```text
Create an explorer agent with fork_context=true and let it analyze src/examples/step09/sandbox.rs first.
```

```text
Create a worker agent and let it continue from the explorer agent’s findings.
```

```text
List the current agents and describe their roles and status.
```

```text
Wait for the explorer agent to finish, then send its result to the worker agent.
```

## One-line summary

The core of `step09` is upgrading the multi-agent control plane into a minimal organized agent team with roles, parent-child relationships, context inheritance, and result propagation.
