# GUI2TUI 当前源码状态、能力与本次改动

更新日期：2026-10-20。对应源码提交：[`76dc5ff`](https://github.com/Chenhzjs/GUI2TUI/commit/76dc5ff96170a4516f4a9a382ea971fe3b57d085)。

该提交已进入 main，包含原生 Agent 接口、Accessibility 执行与回读改进、并行读取、TUI 呈现改进及验证脚本。`Cargo.toml` 版本仍为 `1.1.0-rc.1`；本次源码提交没有创建版本标签或发布二进制包。历史 release 的支持矩阵保持各自原有版本身份。

## 项目定位与整体情况

GUI2TUI 面向**通用桌面 GUI 软件**，将应用通过公共 Linux AT-SPI 暴露的语义、状态、关系和操作能力重新组织为终端交互。浏览器与网页只是其中一类应用和验证场景。生产实现不依赖应用源码，不使用应用专用适配器、预建应用导航图或按品牌分支的规则。

当前有两种使用入口，共享同一语义缓存和操作授权链：

```text
桌面 GUI → 公共 AT-SPI → SemanticCache / 关系与作用域
                         ├─ Region / Content / Spatial → TuiScene → 人类 TUI 操作
                         └─ 公共节点与操作导出 → 决策客户端 → 当前 operation ID
                                    ↓
                   session / generation / scope / 精确对象 / ticket 校验
                                    ↓
                         公共 backend 执行 → 权威回读
```

TUI 路径提供 Surface、Reader、命令和 F3 操作列表；Agent 路径导出结构化 JSON，**不解析终端画面，也不经过 TUI 的内容预览压缩**。Jev-Anything 是独立决策客户端，不是 GUI2TUI 后端或状态权威。本次提交只涉及 GUI2TUI；另一仓库的本地改动未随之提交。

运行目标应用需要 Linux 图形及 Accessibility 会话。可采用已记录的 Managed Xvfb、容器或同主机 SSH 拓扑；终端用户不必直接操作图形桌面。macOS 可做编译与非 live 测试，当前没有原生 macOS Accessibility 后端。具体环境资格以 [limitations](limitations.md) 和相应版本的支持矩阵为准。

## 当前能力

| 能力 | 当前行为 | 边界 |
| --- | --- | --- |
| 应用发现与选择 | 枚举当前 Accessibility 会话中的应用，明确选择目标 | 重名或含糊目标不猜测；仅能看到同会话公开的应用 |
| TUI Surface 与导航 | 按语义区域呈现表单、控件、应用外壳和文档上下文；适配终端宽度 | 布局不改变角色、能力、身份或 modal 作用域 |
| Reader / Outline / Search | 读取公开文本、标题及内容关系，保留 Reader 入口 | 长文档及虚拟化内容可能 partial；不是文件格式解析器 |
| 具名 Action | F3 列出目标当前公开的精确动作名并调用 | 不猜 action[0]，不把任意动作名解释成固定效果 |
| Selection | 根据公共父级 Selection 能力操作成员，以及已支持的单选、表格行等语义 | 成员 selected 回读不保证应用触发业务 change 事件 |
| Value | 对合格的可调整对象按公开范围、步长操作或设有限数值 | 信息型 Value 保持只读；requested/resulting 不一致时不确认请求效果 |
| 文本写入 | 合格单行编辑、明确的完整全文替换；TUI 另有范围编辑和读取选区 | 依赖公开接口、完整性、预算及严格回读；全文替换不保证格式或嵌入内容保真 |
| 动态 GUI 与 modal | 更新当前场景，拒绝失效目标和旧作用域操作 | 呈现中的对象连续性不转移旧授权；不自动恢复并重放旧语义动作 |
| 外部资源与编辑器 | TUI 可使用已配置的资源处理和私有临时文本副本 | 保留现有配置与授权边界；不直接修改应用 backing file 来模拟操作 |
| Agent JSONL 接口 | persistent runtime 内观察状态、选择当前操作 ID、执行并返回回执 | 开发接口；不是远程无认证服务，也不保证所有 AT-SPI primitive 都已导出 |

密码内容不读取，敏感子树的 label、description、value 在 Agent 边界被遮蔽，并且不暴露其操作。生产语义来源不使用 DOM/CDP、OCR、截图理解、坐标点击、私有 GUI API 或应用数据库修改。

TUI 已有显式的 Raw Enter 低层能力，仅在合格 X11 环境中经过当前目标和焦点校验后投递；它不自动充当 Submit。Agent v1 不导出 raw key，也不提供范围编辑、读取文本选区或外部资源 handoff。

## `76dc5ff` 的主要更改

### 1. 增加外部决策客户端入口

新增 [`gui2tui-agent`](../src/bin/gui2tui-agent.rs) 和 [`tui/agent.rs`](../src/tui/agent.rs)。同一连接持有一个 runtime，支持 `observe`、`execute`、`quit`。观察包含 session、generation、interaction scope、节点和 observation-local operation ID。

`execute` 每次尝试都会消费 observation；观察有效期为 120 秒。执行前重新观察并检查精确目标、角色、状态、当前值、祖先、动作、能力及 authority。客户端提交的是当前公开操作 ID，而不是任意 AT-SPI 命令。

### 2. Agent 读取完整公共文本

Agent 对当前作用域内非敏感、公开 Text 的节点读取完整公共文本，不拿终端缓存预览冒充全文。成功标记 `value_complete: true`；失败保留 `value_error`、`value_complete: false` 和 partial，不静默替换成预览。

并行任务按 32 个在途读取调度，仍遍历全部合格公共 Text 节点。并发数量是执行调度，不是最多导出 32 个节点。

### 3. 改进 Unicode 写入的权威证据

新增 [`backend/text_evidence.rs`](../src/backend/text_evidence.rs)。当公共 TextChanged 事件和 fresh Text 回读共同满足严格条件时，可证明完整字面替换，处理部分 provider 的非 BMP/UTF-16 偏移填充问题。

它不通过删除 U+FEFF 或按应用身份修正文本。真实 U+FEFF 仍是内容；证据不完整、流出错或回读不一致时不确认。回执可分别保留验证后的 literal 和原始 exposed text，删除及瞬时事件 payload 不进入回执。

### 4. 分开记录接受、效果和任务完成

具名 action 被后端接受，也可能只有 `effect: unknown`。文本、选择和数值通过各自权威回读确认 primitive；数值请求与结果不同不能当作请求成功。模型选择 `done` 也不等于完整用户任务已完成。

`delivery: unknown` 不自动重放；`not_attempted` 后重新观察和重新决策。此接口不承诺跨进程持久化 exactly-once。

### 5. 并行 Accessibility 元数据读取

[`backend/atspi.rs`](../src/backend/atspi.rs) 在单个节点内并行读取互不依赖的 role、name、description、state、interface 和 children 元数据，保持确定的子节点顺序及 node/depth 计数。并行读取不能将动态应用状态变成原子快照，执行前仍需重新校验。

### 6. 改进 TUI 呈现和操作详情

调整字段单行/双行布局、区域紧凑呈现与滚动、F3 简洁标题和可翻页详情。完整公开标签仍用于搜索；条目保留精确目标身份。同名控件不因显示紧凑而合并绑定，presentation 不制造能力。

### 7. 增加可复现的验证脚本

[`tests/research/web`](../tests/research/web) 包含受控 fixture、direct/决策客户端驱动、protocol 检查、独立公共 AT-SPI oracle、真实网站任务与录制脚本。应用名称、任务选择器和 fixture 标签只属于测试数据，生产代码不导入它们。

操作细节见 [公共操作指南](public-operations.md)；请求、回执与失效规则见 [Agent 协议](agent-protocol.md)。

## 如何使用当前源码

构建需要 Rust 1.88 或更新版本。live 操作需要目标应用运行在同一 Linux Accessibility 会话中。

```sh
cargo build --locked --bin gui2tui --bin gui2tui-agent
./target/debug/gui2tui --session managed
./target/debug/gui2tui-agent --session managed --app 'EXACT ACCESSIBLE APPLICATION NAME'
```

主 TUI 可发现和选择应用，或按 [入门指南](getting-started.md) 注册并启动应用；Agent 当前要求明确给出 `--app`，不会替用户猜测未启动的应用。

Agent 从 stdin 逐行接受 JSON：

```json
{"command":"observe"}
```

随后从实际响应中取得 observation ID 和 operation ID，再发送对应的 `execute`；参数仅在该操作声明需要时提供。操作后重新观察。不要复制示例或历史日志中的 ID 当作当前目标。

默认树发现预算为 10000 个节点、64 层，全文编辑预算为 256 KiB，可分别通过 `--max-nodes`、`--max-depth`、`--max-edit-bytes` 明确配置。超过观察能力或预算时保留 partial/错误。原始应用尚未公开的虚拟化对象不会因提高预算而出现。

独立 Jev 客户端的资源充足实现保留全部已导出的非 redacted 节点和完整候选，不因 provider 预算静默删节点或做候选 tournament。G2T 的发现/编辑预算和合法 scope 仍然独立生效；这不代表已经导出软件所有未来可达功能。

## 验证结果与证据身份

下表严格区分本次提交前检查与较早批次。历史 GUI 结果不冒充在 `76dc5ff` 上重新运行的完整资格验证，也不代表任意桌面软件全部可用。

| 验证 | 结果 | 身份及解释 |
| --- | --- | --- |
| 本次源码提交前 Rust library 单测 | **398/398** | `cargo test --offline --locked --lib`，包含 authority、文本证据和 TUI 回归 |
| 本次源码提交前 Python oracle/selector 单测 | **7/7** | `unittest discover -s tests/research/web -p 'test_*.py' -v` |
| 本次源码提交前二进制检查与格式 | 通过 | `cargo check --offline --locked --bins`、`cargo fmt --all -- --check`、`git diff --check` |
| 历史原冻结 direct | **50/54** | `jev-unicode-fix-20261019-133941/frozen-direct-v2`；保持原计划、顺序和 oracle |
| 历史独立修订 v3 direct | **54/54** | 同 run 的 `revised-v3-modal-safe`；修订了三个 direct 计划和公共文本 oracle，不能替代冻结成绩 |
| 历史真实公开网站 direct | **20/20** | `latency-20261019-175733/parallel/batch`，包含同页多内容及多网站单任务；未调用 provider |
| 历史浏览器 protocol/authority | **11/11** | `capability-research-20261019-233510/protocol.json`，覆盖参数、旧观察、目标替换、modal、敏感信息及只读能力 |
| 历史完整 Jev 输入离线审计 | **20/20 快照完整保留** | 同 research run；使用 stub，无模型/网络/GUI 调用，候选 target 缺失为 0 |

冻结 direct 的四个未通过项不能简单解释为四个仍未修复的 G2T 缺陷：三个涉及原计划的动作路径、祖先消歧或 action 名称假设；另一个涉及原 Unicode oracle 对 raw Text 的比较。修订批次明确记录新条件，没有倒改原冻结任务。

上表 run 名对应维护者工作区 `artifacts/` 下保留的原始证据目录，未随本次文档提交上传。其关键结果、输入/二进制哈希和范围摘录在 [验证摘要](validation/current-source-20261020/summary.json)。已提交的驱动和 manifest 可用于新建独立运行目录重测；摘要不替代完整原始 trace。

Jev-Anything 的单测与 provider 状态属于独立仓库，不计入 GUI2TUI 的 398 项。完整输入版本曾受实际 provider 请求硬上限阻塞；本次没有重跑模型批次或确认该服务现已解除限制。当前不能据 direct、accepted 或 model done 宣称最终 Jev 模型端到端成绩。

## 性能现状

历史 parallel direct 批次使用 Rust 1.88 dev/unoptimized Linux amd64 构建，在 arm64 主机模拟环境运行。记录为：

| 阶段 | 中位耗时 | 样本数 |
| --- | ---: | ---: |
| G2T observe | 7.79 s | 60 |
| G2T execute | 7.05 s | 16 |
| 模型输入构造与证据写盘 | 0.059 s | 20 |
| 模型推理 | **未测** | 0 |

observe 是完整桥接观察时间，包含刷新、公共读取和传输，并非单个 AT-SPI 查询；execute 包括新观察、校验、投递及回读，并非一次 setter 的成本。输入构造时间不是模型思考时间。已有并行优化，当前证据仍是秒级，不能据该环境推导 native release 性能或普遍速度优势。

## 仍然存在的边界与后续重点

- Public AX 是能力上限。未暴露动作、语义不可靠、自绘区域或不完整文档保留 unavailable/read-only/partial，不增加应用特例补齐。
- Agent v1 只是当前支持的操作词汇，尚非所有 TUI 或 AT-SPI primitive 的全集；TUI 与 Agent 能力需分别说明。
- 观察与调用没有应用内部原子事务。session/generation/scope/locator 校验能拒绝已发现的失效，但不证明任意并发修改下绝对无错误投递。
- named action acceptance、primitive effect、完整 task completion 分开。unknown 不构成重复投递许可。
- 真实网页 direct 验证只是桌面应用的一类证据；还需按冻结条件扩展真实桌面软件工作流及模型对照，才能形成更广泛的泛化结论。
- 本次没有重新资格验证原生 x86_64、所有 Wayland/SSH 拓扑或新安装包；已有历史支持矩阵不会因源码提交自动扩大。
- 能力保留算法、降低模型能力要求等仍为研究假设，当前局部审计和单测没有证明这些论文主张。

建议先阅读本总览，再按需要查看 [公共操作](public-operations.md)、[Agent 协议](agent-protocol.md)、[架构](architecture.md)、[兼容性历史](compatibility.md) 和 [限制](limitations.md)。
