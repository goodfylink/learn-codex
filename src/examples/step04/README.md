[English Version](./README.en.md)

# Step04

`step04` 在 `step03` 的基础上加入了第一次“委派”能力，也就是让 agent 可以把某个子任务交给一个子代理去完成。

对应代码：

- [step04.rs](/Users/ycyin/code/rust code/demo/src/examples/step04/step04.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step04/sandbox.rs)

## 这一版实现了什么

这一版保留：

- 工具注册表
- 文件工具
- `update_plan`

同时新增：

- `spawn_sub_agent`
- 子代理专用的最小执行循环
- 支持主 agent 把某个 bounded task 委派出去

## 和 Step03 的区别

相对 `step03`，`step04` 最大的变化是：

1. 不再只有一个 agent 执行所有事
2. 引入 `SubAgentHandler`
3. 主 agent 可以把特定子任务交给另一个 agent loop 去处理

不过这一步的子代理还比较“轻”：

- 它更像一次递归启动的小型 agent loop
- 还不是可持续存在的 agent 实体
- 也没有 `send_input / wait_agent / close_agent`

所以这一步的重点是先理解“委派”本身。

## 功能重点

`step04` 最关键的是多了一种新的执行策略：

- 如果任务简单，主 agent 自己做
- 如果任务比较独立，可以调用 `spawn_sub_agent` 让子代理处理

这为后面的 agent team 奠定了基础。

## 如何运行

```bash
cargo run --bin step04
```

## 如何提问

这一版适合测试“委派子任务”：

```text
把“读取 src/examples/step04/step04.rs 并总结职责”委派给子代理去做
```

```text
请先规划，再把“查看 Cargo.toml 中的依赖”交给子代理完成
```

```text
请调用子代理帮我分析 src/examples/step04/sandbox.rs 里新增了什么
```

## 一句话总结

`step04` 的核心升级，是让 agent 第一次具备“把子任务委派给另一个 agent loop”的能力。
