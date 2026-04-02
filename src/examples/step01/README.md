[English Version](./README.en.md)

# Step01

`step01` 是这个项目的起点，它只实现了一条最小可用的 agent loop：

- 接收用户输入
- 调用模型
- 让模型决定是否调用工具
- 执行工具
- 把工具结果写回上下文
- 再让模型继续完成回复

这一版的目标不是工程化，而是先把“函数调用式 agent”的核心控制流跑通。

## 这一版实现了什么

`step01` 只提供了一个工具：`run_bash`。

对应代码：

- [step01.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/step01.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/sandbox.rs)

从能力上看，这一步已经能做到：

- 维护一份最基础的 `conversation_history`
- 将工具 schema 一起发给模型
- 识别模型返回的 `tool_calls`
- 解析 `run_bash` 参数
- 执行 shell 命令
- 把工具结果作为 `role: tool` 消息写回历史
- 再次请求模型，让模型基于工具观察结果继续推理

这就是后面所有步骤的基础。

## 主流程是怎么跑的

[step01.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/step01.rs) 里的主循环可以概括成下面这几步：

1. 读取环境变量  
   主要包括：
   - `OPENAI_API_KEY`
   - `OPENAI_BASE_URL`
   - `OPENAI_MODEL_NAME`

2. 初始化 system prompt 和对话历史  
   system prompt 会明确告诉模型：
   - 你可以使用 `run_bash`
   - 需要时先调用工具再回答

3. 定义单个工具 schema  
   这一版没有工具注册表，`run_bash` 的 schema 直接写在主文件里。

4. 读取用户输入并追加到 `conversation_history`

5. 发送一次 chat completion 请求  
   请求体中包含：
   - `model`
   - `messages`
   - `tools`
   - `temperature`

6. 处理模型返回结果  
   如果模型给出了 `tool_calls`：
   - 先把 assistant 消息写回历史
   - 取第一条 tool call
   - 解析参数
   - 执行 `run_bash`
   - 把工具输出写成 `role: tool` 消息
   - 继续下一轮请求

7. 如果模型没有调用工具  
   直接把最终文本输出给用户

这个控制流就是最小版的 ReAct / function calling agent loop。

## `sandbox.rs` 做了什么

[sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/sandbox.rs) 负责 `run_bash` 的执行逻辑。

这一层做了 4 件很关键的事：

1. 风险命令过滤  
   会拦截一些明显危险的命令，例如：
   - `rm -rf`
   - `mkfs`
   - `dd if=`
   - `shutdown`
   - `sudo`

2. 超时控制  
   命令执行超过 10 秒会被终止。

3. 输出截断  
   `stdout` 和 `stderr` 太长时会被裁剪，避免一次工具输出把上下文塞满。

4. 结构化结果返回  
   工具最终会返回结构化 JSON 字符串，包含：
   - `ok`
   - `tool`
   - `message`
   - `error_code`
   - `data`

这让模型后续读取工具结果时更稳定，也比早期纯文本 observation 更容易扩展。

## 这一步为什么重要

`step01` 最重要的价值，不是工具数量，而是把下面这条协议跑通了：

- assistant 先发出 tool call
- 本地执行工具
- 再补一条 `role: tool` 消息
- 模型继续基于 observation 思考

只要这条链路通了，后面才能继续往上加：

- 工具注册表
- 计划更新
- 子代理
- skill
- 上下文压缩
- 并发工具
- agent team

## 这一版的限制

作为第一步，它也有明显边界：

- 只支持一个工具 `run_bash`
- 只处理第一条 tool call
- 工具分发是硬编码，不可扩展
- 没有 `update_plan`
- 没有 skill 注入
- 没有上下文压缩
- 没有并发工具调用
- 没有 agent / sub-agent 控制面

所以它更适合用来理解“最小 agent loop 是怎么工作的”，而不是当成完整 runtime。

## 如何运行

如果当前项目根目录是 `/Users/ycyin/code/rust code/demo`，可以直接运行：

```bash
cargo run --bin step01
```

如果这个示例当前还没有单独配置成可执行 target，也可以把它当作源码阅读示例，重点看：

- [step01.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/step01.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step01/sandbox.rs)

## 如何提问

`step01` 只有一个 `run_bash` 工具，所以提问方式最好围绕“让 agent 通过 shell 去查看或执行某件事”。

可以直接这样测试：

```text
帮我查看当前目录是什么
```

```text
列出当前目录下的文件
```

```text
请读取 Cargo.toml 的内容
```

```text
帮我执行 pwd 和 ls -la，然后告诉我你看到了什么
```

这一版更适合问：

- 查看当前目录
- 列出文件
- 读取文件内容
- 执行简单 shell 命令

## 一句话总结

`step01` 做的事情很纯粹：先用一个 `run_bash` 工具，把最小函数调用式 agent loop 跑通，为后面的所有工程化步骤打基础。
