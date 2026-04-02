[English Version](./README.en.md)

# Step06

`step06` 在 `step05` 的基础上，开始对齐 Codex 很重要的一点：同一轮里多个独立工具调用可以并发执行，而不是一个接一个顺序跑。

对应代码：

- [step06.rs](/Users/ycyin/code/rust code/demo/src/examples/step06/step06.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step06/sandbox.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step06/skills.rs)

## 这一版实现了什么

这一版保留了：

- planning
- 子代理
- skill
- 上下文压缩

同时新增：

- `parallel_tool_calls`
- `ToolInvocation`
- `dispatch_many`
- 允许可并发工具批量执行

## 和 Step05 的区别

相对 `step05`，`step06` 的关键升级是：

1. 一轮内可以接收一批 tool calls
2. 注册表开始支持批量分发
3. 工具可以声明自己是否支持并发执行

因此这一步解决的是：

“agent 不只是会调用多个工具，还能更高效地同时调用多个独立工具。”

## 功能重点

这一版里：

- `run_bash`
- `read_file`

被视为更适合并发的工具，而写类工具仍然更偏串行。

所以它开始接近真正 runtime 的工具调度层。

## 如何运行

```bash
cargo run --bin step06
```

## 如何提问

这一版适合测试“并发读 / 并发检查”：

```text
请并行读取 Cargo.toml 和 src/main.rs，然后总结结果
```

```text
请同时查看 src/examples/step05/step05.rs 和 src/examples/step06/step06.rs，说说 step06 多了什么
```

```text
先并行执行 pwd 和读取 Cargo.toml，再告诉我项目结构
```

## 一句话总结

`step06` 的核心，是让 agent loop 从顺序工具执行升级到支持一轮内批量并发工具调用。
