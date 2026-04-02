[English Version](./README.en.md)

# Step05

`step05` 在 `step04` 的基础上，开始处理两个真实 runtime 很快会遇到的问题：

- 对话历史会越来越长
- 某些任务需要按需注入专门知识，而不是把所有知识都塞进 prompt

对应代码：

- [step05.rs](/Users/ycyin/code/rust code/demo/src/examples/step05/step05.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step05/sandbox.rs)
- [skills.rs](/Users/ycyin/code/rust code/demo/src/examples/step05/skills.rs)

## 这一版实现了什么

这一版保留了前面的：

- 规划
- 文件工具
- 子代理委派

同时新增：

- 简化版 `compact_history`
- 本地 skills 扫描
- skill 列表渲染到 system prompt
- 用户显式提到 skill 时按 turn 注入 skill 内容

## 和 Step04 的区别

相对 `step04`，`step05` 最大的变化是：

1. 开始处理长上下文
2. 开始支持 skill
3. system prompt 从固定文案变成“基础指令 + 动态 skills section”

如果一句话总结：

`step04` 解决委派问题，`step05` 开始解决“上下文怎么撑住、知识怎么按需给到模型”的问题。

## 功能重点

### 1. 上下文压缩

`compact_history` 会在历史过长时：

- 保留 system prompt
- 保留最近一段交互
- 把更早的对话总结成摘要

这是一种简化版的 long-context 管理。

### 2. Skill 注入

这一版先扫描本地 `skills/`，再把“有哪些 skill 可用”渲染到 prompt。  
只有当用户显式提到某个 skill 时，才把对应内容注入本轮上下文。

这和后面主实现的思路是一致的。

## 如何运行

```bash
cargo run --bin step05
```

## 如何提问

这一版适合测试 skill 和长上下文能力：

```text
请用 $rust-review 帮我审查 src/main.rs
```

```text
请先读取 src/examples/step05/step05.rs，再总结 skill 是怎么注入的
```

```text
我会连续给你很多信息，之后请继续基于前文完成任务
```

## 一句话总结

`step05` 的核心是把 skill 和上下文压缩接进 agent loop，让系统开始具备长会话和按需知识注入能力。
