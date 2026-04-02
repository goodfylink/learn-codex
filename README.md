# Agent Harness Primer

[English Version](./README.en.md)

> **模型负责决策，Harness 负责让决策变成结果。**
>

---

## Overview

对照 OpenAI 官方目前对 Codex 的公开介绍，可以把 Codex 理解成一个 **AI coding agent**，而不只是会写代码的聊天模型。它强调的能力通常包括：

- 在本地工具里和开发者协作
- 把任务委派到后台环境执行
- 在隔离环境中修改代码、运行命令和测试
- 在需要时并行处理多个任务

所以如果要理解 Codex，重点不只是模型本身，而是：

```text
模型 + 工具 + 上下文 + 状态 + 并发 + 权限边界 + 反馈机制
```

这也是为什么这个项目不会把重点放在“怎么写更长的 prompt”，而是放在“如何逐步搭出一个像 Codex 的 harness/runtime”。

过去两年，围绕大模型出现了很多新词：

- prompt engineering
- context engineering
- agent engineering
- harness engineering
- eval-driven development

如果把这些词放回工程视角，其实可以看到一条很清晰的演进线：

```text
Prompt -> Context -> Tools -> Runtime -> Harness -> Evals
```

这不是单纯的 buzzword 叠加，而是因为问题在一层一层暴露：

- 一开始，大家发现 prompt 写法会显著影响结果。
- 然后，大家发现模型失败常常不是不会想，而是没看到该看的信息。
- 接着，大家发现单次生成不够，模型必须能调用工具、接收反馈、持续行动。
- 再往后，真正的难点变成状态、权限、隔离、恢复、并发和评测。

这份 README 想说明的核心只有一句：

**当模型足够强时，工程竞争力越来越来自 Harness 质量。**

---

## Core Ideas

### Model

模型是底层能力本身。

它负责：

- 理解输入
- 形成判断
- 选择下一步
- 决定是否调用工具
- 决定何时停止或继续

从工程视角看，模型更像一个**决策引擎**。

模型很重要，但模型本身不等于完整系统。一个很强的模型，如果没有正确的上下文、工具、状态和权限边界，仍然可能在真实任务中表现很差。

### Prompting

Prompt 是你给模型的指令表达方式。

Prompt engineering 解决的是：

- 如何把目标说清楚
- 如何减少歧义
- 如何约束输出格式
- 如何提高稳定性

它解决的是输入层的一部分问题，但不是完整系统问题。

### Context Engineering

Context 是模型在某一时刻实际“看到”的信息集合。

Context engineering 关注的是：

- 该给模型什么信息
- 不该给什么信息
- 什么时候按需加载
- 哪些历史要压缩
- 哪些结果要保留原样
- 哪些噪声必须隔离

Prompt 更像“你怎么说”，Context 更像“模型真正看见什么”。

### Agent Loop

Agent 不是一次 API 调用，也不只是一个提示词模板。

一个最小 Agent，至少需要：

- 模型
- 工具
- 循环
- 反馈
- 状态

它能够围绕目标持续行动，而不只是生成一次文本。

### Harness Layer

Harness 是支撑 Agent 在真实环境中工作的工程层。

它可以概括为：

```text
Harness = Tools + Context + State + Memory + Permissions + Feedback
```

其中通常包含：

- **Tools**：文件、Shell、数据库、浏览器、API
- **Context**：任务信息、代码库、文档、历史动作
- **State**：待办、依赖、后台任务、恢复点
- **Memory**：摘要、长期知识、持久化目标
- **Permissions**：沙箱、审批、网络边界、信任范围
- **Feedback**：日志、报错、diff、执行结果、评测信号

Harness 不是替模型思考。Harness 是给模型一个可以工作的世界。

### Evals

Evals 是评估系统行为的机制。

它解决的是：

- 系统是不是变好了
- 新 prompt / 新模型 / 新工具有没有带来退化
- 失败发生在结果层还是过程层
- 一个改动是否破坏了另一个场景

没有 evals，AI 系统就很难形成真正的工程闭环。

---

## Minimal Architecture

无论外层名字怎么变化，很多 Agent 系统的最小核心都可以收敛成下面这个循环：

```python
def agent_loop(messages):
    while True:
        response = model(messages=messages, tools=TOOLS)
        messages.append(response)

        if response.stop_reason != "tool_use":
            return response

        results = []
        for tool_call in response.tool_calls:
            output = TOOL_HANDLERS[tool_call.name](**tool_call.input)
            results.append({
                "type": "tool_result",
                "tool_use_id": tool_call.id,
                "content": output,
            })

        messages.append({"role": "user", "content": results})
```

这个循环很小，但已经足够产生 Agent 行为：

- 模型判断下一步
- 系统执行工具
- 工具结果重新回到上下文
- 直到任务完成

真正的工程复杂度，不在循环本身，而在循环外面那一整层系统机制。

---

## Why Prompting Is Not Enough

Prompt engineering 先流行起来，是因为它抓住了最直接的问题：

**同一个模型，不同写法的指令，效果可能差很多。**

但随着系统进入真实场景，失败的原因通常会变成：

- 模型没看到关键上下文
- 工具接口太粗糙
- 历史信息过载
- 状态没有持久化
- 权限边界不清楚
- 缺少失败恢复
- 无法做评测和回归

这时候，再继续只盯着 prompt，就会越来越不够。

所以行业自然会从 Prompt 走向 Context，再走向 Harness。

---

## Why Context Matters But Is Not the Whole System

Context engineering 是当前非常关键的一层。

因为很多 Agent 的失败并不是“不会推理”，而是：

- 看到的信息不对
- 看到的信息过多
- 看到的信息顺序混乱
- 重要历史被噪声淹没
- 工具结果无法被有效利用

但 Context 仍然只回答了一个问题：

**模型此刻看见什么。**

真实系统还需要回答：

- 模型此刻能做什么
- 执行动作的环境在哪里
- 执行后如何记录与恢复
- 哪些操作需要审批
- 多任务如何并行或隔离
- 如何衡量系统是否真的进步了

这些问题合起来，才进入 Harness 的范围。

---

## Why Systems Like Codex CLI and Claude Code Matter

这类系统的重要性不在于“更像聊天机器人”，而在于它们展示了一个事实：

**当模型已经足够强时，系统效果的差异主要来自 Harness 设计。**

以 coding agent 为例，一个真正有工程价值的系统，通常不只是一次回答，而是包含：

- 文件读取与修改
- 命令执行
- 错误收集
- 结果反馈
- 项目级配置
- 技能或文档加载
- 审批模式
- 沙箱边界
- 持续任务推进

所以像 Codex CLI、Claude Code 这类系统，更接近：

**agent runtime + tools + state + permissions + feedback 组成的 Harness 系统**

而不只是 prompt 或 context 的延伸。

### How This Project Maps To That Evolution

这个项目的意义，就是把这条演进线拆成可单独理解的步骤。

当前主实现文件是：

- `src/main.rs`
- `src/sandbox.rs`
- `src/skills.rs`
- `src/agent_team.rs`

而 `src/examples` 负责按阶段讲清楚能力是怎么长出来的。

#### Step 01: Minimal Agent Loop

文件：

- `src/examples/step01/step01.rs`
- `src/examples/step01/sandbox.rs`

做了什么：

- 建立最小 function-calling loop
- 只支持一个工具 `run_bash`
- 把 tool result 回填到 history 后继续采样

这一阶段对应的是：

```text
Prompt + Tools + Loop
```

#### Step 02: Tool Registry

文件：

- `src/examples/step02/step02.rs`
- `src/examples/step02/sandbox.rs`

做了什么：

- 引入 `ToolRegistry`
- 将工具从主循环中拆出去
- 注册 `run_bash`、`read_file`、`write_file`、`edit_file`

这一阶段开始体现 Harness 的一个关键特征：

**工具不是 if/else，而是受 runtime 管理的能力集合。**

#### Step 03: Planning

文件：

- `src/examples/step03/step03.rs`
- `src/examples/step03/sandbox.rs`

做了什么：

- 新增 `update_plan`
- 让复杂任务先分步骤，再更新状态

这一阶段开始从“会调工具”走向“会组织任务”。

#### Step 04: Delegation

文件：

- `src/examples/step04/step04.rs`
- `src/examples/step04/sandbox.rs`

做了什么：

- 新增 `spawn_sub_agent`
- 第一次支持把子任务委派给另一个 agent loop

这一阶段的 sub-agent 还很轻量，更像递归式子调用，但已经体现出：

**agent runtime 需要支持 delegation，而不只是单线程执行。**

#### Step 05: Skills And Context Compaction

文件：

- `src/examples/step05/step05.rs`
- `src/examples/step05/sandbox.rs`
- `src/examples/step05/skills.rs`

做了什么：

- 加入 `compact_history(...)`
- 扫描本地 skills
- 将 skill 列表渲染进 prompt
- 当用户显式提到 skill 时，按 turn 注入 `SKILL.md`

这一阶段解决了两个典型 Harness 问题：

- 长上下文怎么撑住
- 外部知识怎么按需加载

#### Step 06: Parallel Tool Calls

文件：

- `src/examples/step06/step06.rs`
- `src/examples/step06/sandbox.rs`
- `src/examples/step06/skills.rs`

做了什么：

- 加入 `parallel_tool_calls`
- 一轮里批量执行多个 tool calls
- 读类和执行类工具开始支持并发调度

这一步开始对齐 Codex 很重要的一点：

**真正的 coding agent 不会把所有动作都串行化。**

#### Step 07: Agent Team Part 1

文件：

- `src/examples/step07/step07.rs`
- `src/examples/step07/sandbox.rs`
- `src/examples/step07/skills.rs`
- `src/examples/step07/agent_team.rs`

做了什么：

- 定义 `AgentThread`
- 定义 `AgentTeamManager`
- 让 agent 先变成一个可管理实体

这一步回答的是：

**一个 agent 在 runtime 里到底是什么？**

#### Step 08: Agent Team Part 2

文件：

- `src/examples/step08/step08.rs`
- `src/examples/step08/sandbox.rs`
- `src/examples/step08/skills.rs`
- `src/examples/step08/agent_team.rs`

做了什么：

- 新增 `spawn_agent`
- 新增 `send_input`
- 新增 `wait_agent`
- 新增 `close_agent`
- 新增 `list_agents`
- 增加输入队列、状态订阅、后台 worker

这一步让 agent 不只是对象，而是可持续协作的 worker。

#### Step 09: Agent Team Part 3

文件：

- `src/examples/step09/step09.rs`
- `src/examples/step09/sandbox.rs`
- `src/examples/step09/skills.rs`
- `src/examples/step09/agent_team.rs`

做了什么：

- 引入 `AgentRole`
- 引入 `ToolExecutionContext`
- 引入 `parent_agent_id`
- 引入 `depth`
- 引入 `fork_context`
- explorer role 有最小权限边界
- child agent 完成后会自动回写 parent

这一步才真正接近 Codex 的 team 语义：

**team 不是多开几个 agent，而是有角色、层级、上下文继承和结果回报机制。**

### What `src/main.rs` Represents

当前 `src/main.rs` 可以理解为：

```text
step05 + step06 + step07 + step08 + step09 的合并版主实现
```

它已经包含：

- tool calling loop
- 结构化工具返回
- skill 注入
- 上下文压缩
- 并发工具调用
- sub-agent delegation
- agent team 三层能力

所以如果你想学演进过程，优先看 `examples`。

如果你想看当前完整跑法，优先看：

- `src/main.rs`
- `src/sandbox.rs`
- `src/agent_team.rs`

---

## What Harness Engineers Actually Build

从这个角度看，所谓 Harness 工程师，真正负责的大致是下面这些工作。

### Tooling

为模型提供行动能力：

- 读写文件
- 执行命令
- 搜索代码
- 调数据库
- 调浏览器
- 调业务 API

好的工具通常具备三个特征：

- **原子**：一次只做一件明确的事
- **清晰**：模型知道何时使用、输入输出是什么
- **可组合**：多个工具可以自然串联成更复杂流程

### Context Management

为模型提供高质量输入：

- 当前任务需要什么信息
- 哪些信息应该按需加载
- 哪些历史需要压缩
- 哪些结果必须保真
- 哪些噪声必须隔离

### State and Memory

让系统能够持续运行：

- 待办任务
- 依赖关系
- 后台任务
- 长期目标
- 恢复点
- 子任务结果

### Permissions and Safety

让系统可以自动化，但不失控：

- 哪些目录可写
- 哪些命令必须审批
- 是否允许联网
- 是否能接触生产资源
- 哪些操作必须保留人工确认

### Feedback and Evaluation

让系统可以被改进：

- 日志
- 报错
- diff
- 成本
- 延迟
- 成功率
- 回归评测

换句话说，Harness 工程师并不是在“手写智能”，而是在为智能搭建工作环境。

---

## Why Evals Cannot Be An Afterthought

如果系统开始具备：

- 多工具
- 长上下文
- 多轮行动
- 并发
- 子代理
- 团队协作

那么任何一个小改动都可能带来退化。

例如：

- 一个 prompt 改动可能让 planner 变差
- 一个工具 schema 改动可能破坏旧调用
- 一个压缩策略改动可能让 agent 丢掉关键历史
- 一个子代理权限边界改动可能让任务无法完成

所以 evals 不是最后补的东西，而是让 Harness 具备工程可维护性的前提。

---

## How To Read This Demo

推荐顺序：

1. `src/examples/step01/step01.rs`
2. `src/examples/step02/step02.rs`
3. `src/examples/step03/step03.rs`
4. `src/examples/step04/step04.rs`
5. `src/examples/step05/step05.rs`
6. `src/examples/step06/step06.rs`
7. `src/examples/step07/agent_team.rs`
8. `src/examples/step08/agent_team.rs`
9. `src/examples/step09/agent_team.rs`
10. `src/main.rs`
11. `src/sandbox.rs`

如果压成三个阶段：

- 基础 agent runtime：`step01` 到 `step04`
- runtime 增强：`step05` 到 `step06`
- agent team：`step07` 到 `step09`

---

## How To Run

在项目根目录运行：

```bash
cargo run
```

如果你在父目录中，也可以显式指定 manifest：

```bash
cargo run --manifest-path ./Cargo.toml
```

常用环境变量：

```bash
export OPENAI_API_KEY=your_key
export OPENAI_BASE_URL=your_endpoint
export OPENAI_MODEL_NAME=your_model
```

启动后输入任务，退出用：

```text
exit
```

你可以这样试：

```text
请读取 src/main.rs，然后总结主循环在做什么
```

```text
请用 $rust-review 审查 src/sandbox.rs
```

```text
创建一个 explorer agent，fork_context=true，让它分析 src/sandbox.rs 的工具调度逻辑
```

```text
再创建一个 worker agent，根据 explorer 的结论继续执行实现任务
```

---

## Current Limitations

这个项目仍然是教学用实现，不是生产级 runtime。

目前的简化点包括：

- 上下文压缩是启发式实现，不是 token 级治理
- 安全策略仍然是教学版实现级别
- `fork_context` 是简化版历史继承
- `close_agent` 不是强中断当前 HTTP 请求
- role 权限边界只实现了最小版
- 没有完整的事件系统、恢复系统和评测体系

但对于理解 Codex 风格 Harness 的核心结构，这套项目实现已经足够。
