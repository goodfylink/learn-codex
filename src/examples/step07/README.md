[English Version](./README.en.md)

# Step07

`step07` 是 agent team 的第一部分：先把 agent 从一次性的递归子调用，升级成一个真正可管理的实体。

对应代码：

- [step07.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/step07.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/sandbox.rs)
- [agent_team.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/agent_team.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step07/skills.rs)

## 这一版实现了什么

这一版保留了前面的：

- planning
- skill
- 上下文压缩
- 并发工具

同时新增了 agent 实体层：

- `AgentStatus`
- `AgentSnapshot`
- `AgentThread`
- `AgentTeamManager`

## 和 Step06 的区别

相对 `step06`，`step07` 最大的变化是：

1. 子代理不再只是临时递归执行
2. agent 被抽成独立实体
3. 系统开始维护 agent 快照和状态

这一步还没有完整控制面，但已经完成了第一步：

“一个 agent 是什么”。

## 功能重点

这一版最关键的不是多了什么工具，而是多了 agent 数据模型：

- agent 有自己的 `id`
- agent 有自己的 `role`
- agent 有自己的 `history`
- agent 有自己的 `status`

这为后面的 `send_input / wait_agent / close_agent` 做好了铺垫。

## 如何运行

```bash
cargo run --bin step07
```

## 如何提问

这一版适合测试“子代理已经变成实体”：

```text
请把“读取 src/examples/step07/step07.rs 并总结职责”委派给子代理
```

```text
请调用子代理分析 src/examples/step07/agent_team.rs 的作用
```

```text
帮我看看 step07 里 agent 实体比 step06 多了什么
```

## 一句话总结

`step07` 的核心，是把子代理从临时执行流程升级成真正的 agent 实体，这是 agent team 的第一步。
