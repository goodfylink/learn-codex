[English Version](./README.en.md)

# Step09

`step09` 是 agent team 的第三部分：让一组可协作的 agent，真正变成“有组织结构的团队”。

对应代码：

- [step09.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/step09.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/sandbox.rs)
- [agent_team.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/agent_team.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step09/skills.rs)

## 这一版实现了什么

这一版在 `step08` 控制面的基础上，继续补了团队组织能力：

- `AgentRole`
- `parent_agent_id`
- `depth`
- `fork_context`
- 父子 agent 协议
- explorer / worker / default 角色区分
- 子 agent 结果回写父 agent

## 和 Step08 的区别

相对 `step08`，`step09` 最大的变化是：

1. agent 有了正式 role，而不只是字符串标签
2. agent 之间有父子关系
3. 子 agent 可以带着父 agent 的上下文启动
4. 子 agent 完成或失败后，可以自动回写给父 agent

如果一句话总结：

`step08` 解决“agent 怎么协作”，`step09` 解决“agent team 怎么组织起来”。

## 功能重点

### 1. Role

这一版支持：

- `default`
- `explorer`
- `worker`

其中 `explorer` 已经有最小权限边界，更适合只读分析类任务。

### 2. fork_context

创建子 agent 时可以选择是否继承父 agent 的上下文。  
这让 child agent 在需要时能带着背景启动。

### 3. 父子回写

子 agent 完成、失败或关闭后，父 agent 可以收到更新信息。  
这让 team 不再是一组散开的 agent，而是一个有反馈闭环的协作结构。

## 如何运行

```bash
cargo run --bin step09
```

## 如何提问

这一版适合测试 role、fork_context 和团队协作：

```text
创建一个 explorer agent，fork_context=true，让它先分析 src/examples/step09/sandbox.rs
```

```text
再创建一个 worker agent，让它根据 explorer 的结果继续完成后续实现建议
```

```text
请列出当前 agents，并说明它们的角色和状态
```

```text
等待 explorer agent 完成，然后把它的结果继续交给 worker agent
```

## 一句话总结

`step09` 的核心，是把多 agent 控制面升级成一个带角色、父子关系、上下文继承和结果回写机制的最小 agent team。
