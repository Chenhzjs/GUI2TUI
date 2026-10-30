# 可选 Jev 客户端

GUI2TUI 是主仓库，`integrations/jev-anything` 是指向
[Chenhzjs/Jev-Anything](https://github.com/Chenhzjs/Jev-Anything) 的可选 Git
submodule。Jev 客户端保持独立历史，根仓库固定它的确切提交。普通 Rust 构建和
Human TUI 不依赖 Python、模型密钥或子模块初始化。

## 获取与安装

已有 GUI2TUI checkout：

```sh
git submodule update --init -- integrations/jev-anything
```

或者首次获取两个仓库：

```sh
git clone --recurse-submodules https://github.com/Chenhzjs/GUI2TUI.git
```

在 Python 3.10+ 的虚拟环境中安装客户端及离线 token 计数依赖：

```sh
python -m pip install -e 'integrations/jev-anything[tui]'
jev-anything-tui --help
```

`--tokenizer-json` 指向用户提供、与所选模型相容的离线 `tokenizer.json`。
仓库不包含分词器文件，不自动下载，也不会在计数不可用时绕过输入预算。
通过环境配置 `DEEPSEEK_API_KEY`；不要把密钥写进命令参数或任务 trace。
当前 TUI 客户端使用 DeepSeek，完整文档比较入口 `jev-anything-native`
则保持显式 provider 选择。

## 连接原 GUI

在运行目标 GUI 的 Linux 主机或容器中构建桥接程序：

```sh
cargo build --release --locked --bin gui2tui --bin gui2tui-agent
```

桥接程序须使用目标应用所在的桌面、D-Bus 和 Accessibility 会话。
在已配置的同一会话中运行，例如：

```sh
jev-anything-tui \
  --goal '完整的用户目标' --text '明确提供的非秘密字面值' \
  --tokenizer-json /path/to/tokenizer.json \
  --max-steps 12 --max-actions 12 \
  --max-prompt-tokens 12000 --max-input-tokens 40000 \
  --trace /path/to/new-private-trace.jsonl \
  --bridge ./target/release/gui2tui-agent --session desktop --app 'Observed app' --spatial-tui
```

trace 必须使用新路径，其父目录应由用户私有保存。模型读取实际 Human TUI 文字和
少量公开状态、操作引用；完整文档保留在本地，屏外内容通过 Rust 导航读取。
这属于部分视图输入，不能等同完整上下文模式。追加式消息保留旧前缀，默认最多
12 次模型决策和 12 个操作尝试；一次 batch 最多 4 个本次已公开对象。
缓存命中、模型 done、动作回读与独立任务完成各自记录。

客户端也可在宿主机运行，经显式配置的 `docker exec -i` 调用同一 GUI 容器中的
桥接程序；`--bridge` 后是完整桥接命令。Jev 的 Dockerfile 固定了兼容的 GUI2TUI
源提交并包含 `gui2tui-agent`，但旧 workbench 与新 TUI runner 是不同入口。
不能仅在旧网页模型菜单中切换后，就认为使用了这条新链路。

## 录制、接管与版本维护

### 原生终端 F7 工作流中心

在 Scene 中按 **F7**，进入可选 Jev 终端工作流中心。它使用同一 Rust renderer
生成的当前 TUI 文字，按终端尺寸浏览，并提供录制与结果菜单。当前文本编辑或其他
覆盖界面需先关闭。普通 Human TUI 不需要安装 Jev；工作流客户端缺失时会给出提示。
本入口要求同次源码构建的 `gui2tui` 和 `gui2tui-agent`。本轮没有改动既有 Docker
构建 pin 或部署服务，旧二进制不包含这个入口。

此入口需要提供 `jev-anything-console` 的兼容客户端版本。当前父仓库固定的
Jev 提交尚不包含该 console 命令；本地开发 checkout 已有实现，但这次仅推送
GUI2TUI，不会把 Jev 的未提交改动混入发布说明。只初始化当前固定子模块不足以
启用 F7；普通 Human TUI、F3 和 Reader 不受影响。
已安装兼容 console 客户端时，可以显式指定入口：

```sh
gui2tui --session desktop --app 'Observed app' \
  --workflow-client /path/to/venv/bin/jev-anything-console
```

1. 选择“开始录制”，填写流程名称和目标。
2. 选择当前公开操作，核对定位名称、上下文、非秘密输入及本步结束条件，再明确执行。
   控件定位使用 Rust 的 `find_control`；F3 操作和命令也有显式入口。同名多对象保持歧义。
3. “保存草稿”保留实际执行和公开检查点证据。草稿尚不能回放。
4. “审阅并确认草稿”逐项显示操作、参数槽位和结束条件。确认后另存已确认流程；
   草稿若在审阅后被改动，确认会被拒绝。
5. 选择已确认流程，提供本次非秘密参数，核对初态后明确发起回放。参数值不写入
   已确认流程，保存的是参数槽位。每一步重新观察、定位并经过 G2T 权威执行链。

这条日常录制与回放路径调用模型 **0 次**。导航不激活 GUI。录制时重新定位的对象
必须与用户刚选的精确公开来源一致；回放没有历史 operation ID 或匿名动作兜底。
目标不可区分、来源变化或检查点不能成立时，会记录原因并停止。原语效果 unknown
不能满足“原语回读已确认”的检查点；其他公开检查点需实际成立，同一未知动作不重放。
UI 不通过 accepted 或控件名称推断任务成功。
首版保存流程要求控件有公开原名，并能由 Rust 选择器结合上下文唯一定位；未命名或
无法区分的对象不会用运行时编号生成持久定位。原生 F3 中的公开能力保持原有行为。
当前参数表单按行输入；已有 API/JSON CLI 继续支持完整字面值供给。
交互终端使用 Python 标准 `readline` 行编辑，避免内核普通按行缓冲造成长输入截断。
输入不加入行编辑历史；实际执行证据和公开回读仍保留在私有记录中。

运行结果显示已通过的步骤、停止位置、原因及未执行余项。结果菜单可以查看历史记录。
“核对整体任务结果”单独保存用户声明，独立任务完成字段保持原义；流程确认、模型结束、
人工恢复和用户核对分别记录。原生界面返回后也显示最近流程结果并重新读取 GUI。

默认目录是 `$XDG_DATA_HOME/gui2tui/workflows`，未设置时使用
`~/.local/share/gui2tui/workflows`。可用 `GUI2TUI_WORKFLOW_DIR` 指定目录。
目录须由当前用户私有持有，记录以随机文件名和 0600 权限新建，不覆盖旧记录。
草稿和运行证据可能包含公开应用内容，应保存在私有位置。

录制人工步骤时必须声明恢复后的公开控件或 Reader 文本检查点。通过
`GUI2TUI_HUMAN_VIEW_FILE` 配置已有的私有桌面入口文件，用户在同一原 GUI 中操作，
然后输入 `RESUME` 或 `CANCEL`。入口文件内容不会读入流程或模型；恢复后重新观察，
不合成 GUI 回执。这里没有创建或部署桌面服务。

F7 期间原生主界面交出终端并停止自身交互，客户端桥接只选择原应用的精确公共
BackendLocator；应用消失时拒绝按名字换绑。客户端关闭其桥接后，原生界面恢复终端、
核对原应用并刷新。两个 CLI 使用独立 runtime，没有新增 daemon 或共享旧授权。

AI TUI CLI 另提供 `--progress`，将步骤、回执、用量和停止原因输出到 stderr，stdout
继续保留 JSON；没有更改模型输入或追加式缓存前缀。

- `jev-anything-workflow` 提供录制、审阅确认和零模型回放；不会自动匹配相似任务。
- `jev-anything-human-view` 是显式配置的原桌面接管工具。远程查看依赖和端口需要
  用户配置；密码与视觉验证留在原 GUI，恢复后重新观察。
- [客户端 README](https://github.com/Chenhzjs/Jev-Anything/blob/main/README.md)、
  [TUI 协议与预算](https://github.com/Chenhzjs/Jev-Anything/blob/main/docs/TUI_READER.md) 和
  [工作流与人工接管](https://github.com/Chenhzjs/Jev-Anything/blob/main/docs/WORKFLOWS.md)
  可在 fork 网页阅读；初始化子模块后也可本地阅读其固定版本。
- [G2T Agent 协议](agent-protocol.md) 由 Rust 核心维护；Python 不复制语义组织，
  模型输出不赋予操作权限。unknown 不自动重放，批量不是原子事务。

更新客户端时，在子模块中检出已发布且兼容的提交，再提交父仓库 gitlink。
不要以 `git submodule update --remote` 代替固定版本的常规安装，也不要将现有
独立开发 checkout 移入或覆盖到子模块目录。Jev Docker 的 GUI2TUI 构建 pin
需要单独审阅，不能由父仓库 gitlink 自动推导。
