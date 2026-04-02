[English Version](./README.en.md)

# Step08

`step08` 是 agent team 的第二部分：让这些 agent 实体之间真正开始协作，而不只是“存在”。

对应代码：

- [step08.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/step08.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/sandbox.rs)
- [agent_team.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/agent_team.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step08/skills.rs)

## 这一版实现了什么

这一版新增了最小 agent 控制面：

- `spawn_agent`
- `send_input`
- `wait_agent`
- `close_agent`
- `list_agents`

同时 agent 实体本身也增强了：

- 输入队列
- 状态订阅
- 完成结果 / 错误记录
- 后台 worker

## 和 Step07 的区别

相对 `step07`，`step08` 最大的变化是：

1. agent 不只是有 `id` 和 `status`
2. agent 可以接收后续输入
3. agent 可以被等待、关闭和列举

也就是说，这一步完成了 agent team 的第二个问题：

“这些 agent 之间怎么协作？”

## 功能重点

这一版已经非常接近最小控制面：

- 创建一个后台 agent
- 给这个 agent 继续派任务
- 等它完成
- 查看当前有哪些 agent
- 关闭某个 agent

所以它开始像一个真正的多 agent runtime 了。

## 如何运行

```bash
cargo run --bin step08
```

## 如何提问

这一版适合测试完整协作流程：

```text
创建一个 agent，让它读取 src/examples/step08/step08.rs 并总结职责
```

```text
列出当前 agents，然后给刚才那个 agent 再发一个 follow-up，让它继续分析 sandbox.rs
```

```text
等待那个 agent 完成，如果 5 秒内没完成就先返回状态
```

```text
关闭刚才那个 agent
```

## 一句话总结

`step08` 的核心，是把 agent 实体升级成可创建、可继续发任务、可等待、可关闭的最小协作控制面。
