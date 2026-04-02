[English Version](./README.en.md)

# Step03

`step03` 在 `step02` 的基础上加入了“计划”这层能力，让 agent 不只是调用工具执行任务，还能把复杂任务拆成步骤并持续更新进度。

对应代码：

- [step03.rs](/Users/ycyin/code/rust code/demo/src/examples/step03/step03.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step03/sandbox.rs)

## 这一版实现了什么

这一版保留了 `step02` 的工具注册表和文件工具，同时新增：

- `update_plan` 工具
- 更明确的 system prompt 约束
- 让复杂任务先规划、再执行的基本流程

当前工具包括：

- `run_bash`
- `read_file`
- `write_file`
- `edit_file`
- `update_plan`

## 和 Step02 的区别

相对 `step02`，`step03` 的关键升级是：

1. 不再只关注“怎么执行”，开始关注“怎么组织任务”
2. 引入 `PlanHandler`，让计划成为显式工具调用
3. system prompt 明确要求复杂任务先拆步骤，再标记 `in_progress` 和 `completed`

如果一句话总结：

`step02` 让工具系统可扩展，`step03` 让 agent 开始具备最小任务编排能力。

## 功能重点

这一版的重点不是新增很多底层能力，而是让 agent 形成更稳定的工作方式：

1. 先更新计划
2. 再开始执行
3. 在执行过程中继续更新状态

这让后面的委派、并发和多 agent 协作更容易接上。

## 如何运行

```bash
cargo run --bin step03
```

## 如何提问

这一步适合测试“先规划再执行”的行为，可以这样问：

```text
请先制定一个计划，再帮我分析这个项目的目录结构
```

```text
请把“查看 Cargo.toml、读取 step03 代码、总结职责”拆成几个步骤来完成
```

```text
请先列计划，再读取 src/examples/step03/step03.rs，并总结它相对 step02 多了什么
```

## 一句话总结

`step03` 的核心是把 `update_plan` 纳入 agent loop，让 agent 从“会用工具”升级到“会先规划再执行”。
