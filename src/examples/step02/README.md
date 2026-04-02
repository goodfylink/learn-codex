[English Version](./README.en.md)

# Step02

`step02` 的目标，是在 `step01` 已经跑通最小 agent loop 的基础上，解决一个很快就会暴露出来的问题：

`step01` 里的工具调用逻辑几乎都写死在主循环里，不方便继续扩展。

所以 `step02` 的核心升级不是“多了几个工具”这么简单，而是把“工具定义、工具 schema、工具执行”从主流程里拆出来，形成了一个最小工具注册表。

## 这一版实现了什么

对应代码：

- [step02.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/step02.rs)
- [sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/sandbox.rs)

这一版已经具备这些能力：

- 使用 `ToolRegistry` 统一管理工具
- 用 `ToolHandler` trait 抽象每个工具
- 自动导出所有工具的 schema 给模型
- 通过注册表分发工具调用
- 支持多个基础工具，而不再只有 `run_bash`

当前 `step02` 提供的工具包括：

- `run_bash`
- `read_file`
- `write_file`
- `edit_file`

也就是说，到这一步，agent 已经从“只能调 shell”升级成了“能读文件、写文件、编辑文件”的基础文件操作 agent。

## 和 Step01 的区别

相对 [step01](/Users/ycyin/code/rust code/demo/src/examples/step01/README.md)，`step02` 的升级点主要有 4 个：

1. 工具不再硬编码在主循环里  
   `step01` 里主循环直接判断：
   - 工具名是不是 `run_bash`
   - 参数怎么解析
   - 工具怎么执行

   `step02` 则改成：
   - 工具先注册进 `ToolRegistry`
   - 主循环只负责把 `function_name` 和 `arguments_json` 交给 registry
   - 真正执行由具体 handler 决定

2. 工具数量从 1 个变成 4 个  
   `step01` 只有 `run_bash`。  
   `step02` 新增了：
   - `read_file`
   - `write_file`
   - `edit_file`

3. 工具 schema 由注册表统一导出  
   在 `step01`，工具 schema 是主文件里写死的一段 JSON。  
   在 `step02`，每个工具自己实现 `spec()`，最后由 `registry.get_specs()` 聚合。

4. 主循环更像 runtime，而不是示例脚本  
   这一步之后，主循环主要负责：
   - 维护历史
   - 请求模型
   - 判断是否调用工具
   - 把调用交给 registry

   具体工具细节已经被移出主流程。

所以如果一句话总结：

`step01` 解决的是“agent loop 能不能跑通”，`step02` 解决的是“这个 loop 能不能扩展”。

## 主流程是怎么变化的

[step02.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/step02.rs) 里的整体控制流和 `step01` 类似，但结构更清晰了：

1. 初始化模型客户端和 system prompt
2. 创建 `ToolRegistry`
3. 注册所有工具 handler
4. 从 registry 导出 `tools_definitions`
5. 用户输入进入 `conversation_history`
6. 请求模型
7. 如果模型返回 `tool_calls`
   - 先保存 assistant 消息
   - 逐个处理 tool call
   - 通过 `registry.dispatch(...)` 分发
   - 把工具结果写回 `role: tool`
8. 如果没有工具调用
   - 直接输出最终回复

这里最重要的变化是：

主循环不再知道每个工具的内部实现，它只知道“把调用交给 registry”。

## `sandbox.rs` 做了什么

[sandbox.rs](/Users/ycyin/code/rust code/demo/src/examples/step02/sandbox.rs) 在这一版里承担了两类职责。

### 1. 定义工具抽象层

这里引入了：

- `ToolHandler`
- `ToolRegistry`

`ToolHandler` 约定每个工具都要提供：

- `name()`
- `spec()`
- `handle(arguments)`

`ToolRegistry` 则负责：

- 注册工具
- 保存工具表
- 根据工具名分发执行
- 聚合所有工具 schema

这就是 `step02` 最核心的工程化升级。

### 2. 实现四个基础工具

这一版的四个 handler 分别是：

- `RunBashHandler`
- `ReadFileHandler`
- `WriteFileHandler`
- `EditFileHandler`

它们都遵守同一套模式：

1. 定义参数结构体
2. 提供 tool schema
3. 解析 arguments JSON
4. 执行具体逻辑
5. 返回结构化 JSON 结果

这种统一模式，为后面继续加工具打下了很好的基础。

## 这一版的功能重点

`step02` 的重点不只是“多了文件操作”，还包括下面两件更重要的事：

1. 可扩展性明显提升  
   以后再加一个工具，只需要：
   - 新建 handler
   - 实现 `ToolHandler`
   - 注册进 registry

   不用再去改主循环里的分支逻辑。

2. 责任分层更清楚  
   - `step02.rs`：负责 agent loop
   - `sandbox.rs`：负责工具抽象和工具执行

这种分层已经开始接近真正 runtime 的组织方式。

## 这一版还没有做什么

虽然 `step02` 比 `step01` 更像工程代码了，但它仍然是一个早期版本，还没有这些能力：

- 没有 `update_plan`
- 没有子代理委派
- 没有 skill 注入
- 没有上下文压缩
- 没有并发工具调用
- 没有 agent team

所以 `step02` 的定位很明确：

它是“工具系统抽象化”的那一步，而不是完整 agent runtime。

## 如何运行

现在这个项目已经把 `step02` 注册成独立 bin target，可以直接运行：

```bash
cargo run --bin step02
```

## 如何提问

`step02` 比 `step01` 多了文件类工具，所以提问可以开始从“只看 shell”扩展到“让 agent 直接读写文件”。

可以直接这样测试：

```text
请读取 Cargo.toml，然后告诉我这个项目依赖了什么
```

```text
请查看 src/examples/step02/step02.rs，并总结这个文件的职责
```

```text
在当前目录创建一个 notes.txt，内容写 hello from step02
```

```text
请把 notes.txt 里的 hello 改成 hi
```

```text
先列出当前目录文件，再读取 Cargo.toml，最后总结这个项目是什么
```

这一版更适合问：

- 读取某个文件
- 创建简单文件
- 修改文件内容
- 结合 shell 和文件工具完成简单任务

相对 `step01`，你现在可以更明确地提“读文件”“写文件”“改文件”，而不必都绕成 shell 命令。

## 一句话总结

`step02` 相对 `step01` 最关键的升级，是把单工具硬编码逻辑改成了“工具注册表 + 多工具 handler”的结构，让整个 agent loop 开始具备可扩展性。
