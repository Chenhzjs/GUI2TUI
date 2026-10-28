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
cargo build --release --locked --bin gui2tui-agent
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
