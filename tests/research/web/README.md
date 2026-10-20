# GUI2TUI + JEV 网页批量验证

[重开发交接与复测基线](../../../docs/jev-redevelopment-handoff.md) 集中记录当前选择问题、
历次测试、逐项任务、完整复跑步骤及带 SHA256 清单的冻结测试附件。

本目录是开发测试脚本与验证网页，使用已有原生 bridge 和 Jev-Anything client。
生产代码不导入测试任务、网页标签或事件断言。所有 GUI 写入只经 G2T 当前公开操作 ID；
HTTP 事件与独立 pyatspi 读取是断言，不是第二条执行路径。像素只用于视频展示。

## 环境与构建

macOS 主机需要 Docker，以及独立仓库 `~/Document/Jev-Anything` 的 Python 环境。
`MIYANG_API_KEY` 只留在主机环境中。Linux 使用现有测试或录制镜像（包含 Firefox、
AT-SPI、D-Bus、Xvfb；录制镜像还包含 Openbox、xterm、ffmpeg）。

```sh
docker build --platform linux/amd64 -t gui2tui-recording:local tests/research/web
docker run --rm --platform linux/amd64 -v "$PWD:/work" -w /work \
  -e RUSTUP_TOOLCHAIN=1.88.0 -e CARGO_TARGET_DIR=/work/target/autonomous-linux \
  rust:1.88-bookworm cargo build --locked --bins
PY=~/Document/Jev-Anything/.venv/bin/python
```

Dockerfile 提供后续复现的必要依赖；本轮实际已有镜像的精确身份另存于各批次 summary。

## 单页连续功能测试

同一个真实 Firefox 页面、同一个 persistent bridge，执行 54 项有状态功能任务：
表单/富文本/空串/Unicode、按钮、复选和单选、多选成员、数值、details、tabs、
modal、动态控件、延迟任务、表格、任务列表、fragment 和 iframe。

```sh
$PY tests/research/web/run.py --image gui2tui-recording:local \
  --output docs/validation/my-run/direct
$PY tests/research/web/run.py --image gui2tui-recording:local --mode jev \
  --max-steps 1 --output docs/validation/my-run/jev
```

direct 不是硬编码后端：它按测试清单在当前原生导出中寻找一个精确操作 ID，再经相同
G2T authority/backend 执行，用于隔离决策失败和执行失败。jev 使用相同生产候选转换
和 `choose`/`run`。这里每项是有意限制为一次决策的原子任务，因此 `step_limit`
不等于任务失败；独立 oracle 的 `task_completed` 才是该项测试结果。前置任务失败可能
影响后续任务，不把连续场景的比例当独立样本统计。多步任务请提高 `--max-steps`。

## 公开网页和协议检查

```sh
$PY tests/research/web/public.py --image gui2tui-recording:local \
  --output docs/validation/my-run/public
$PY tests/research/web/public.py --image gui2tui-recording:local \
  --tasks cargo-guide internet-number --output docs/validation/my-run/targeted
$PY tests/research/web/protocol.py --container EXISTING_WEB_TEST_CONTAINER \
  --output docs/validation/my-run/protocol.json
```

公开清单有 20 项任务，分布于 Python/Rust/Cargo/MDN/Wikipedia/GitHub/TOML、
The Internet、TodoMVC、Example Domain 等。`public.py` 的多步决策与所有回读有完整
记录。模型 done、接受和任务完成分别保留；已加载、既有状态也不能冒充新写入。

## 视频

```sh
$PY tests/research/web/record.py --output artifacts/my-film
python3 tests/research/web/finish_video.py --recording artifacts/my-film
```

录制原 GUI、未改写的 Rust GUI2TUI PTY 输出和真实 JEV choice/receipt/oracle。
`record_inside.py` 只摆放窗口和录屏，不驱动 GUI 功能。任务数据来自 host client。
生成 raw.mp4、未改写的终端字节、decision JSONL、独立断言与 timeline；不录 API Key。
后期标题/字幕仅解释流程，不替换状态、决策或回读。

真实公开网页录制复用 Jev-Anything 的生产 `native.run` 循环：

```sh
$PY tests/research/web/record_public.py \
  --goal 'Enter Accessibility into the Search Wikipedia field and press the Search button to navigate to the article.' \
  --output artifacts/my-wikipedia-film
python3 tests/research/web/finish_video.py --recording artifacts/my-wikipedia-film \
  --output docs/demo/my-wikipedia-film
```

Wikipedia 的最终断言同时要求公共 document 标题和浏览器地址栏中的英文条目 URL。
`--focus-label` 只用于录制准备阶段的只读 TUI 导航，不约束模型的候选或驱动网页操作。

[本轮结果与问题](../../../docs/validation/jev-web-20261017/REPORT.md)

## TUI 可读性对照

`capture_tui.py` 采集实际 PTY；`compare_tui.py` 在同一 Accessibility 会话中运行新旧
GUI2TUI，并把原始 PTY 字节镜像到两个 xterm，截取前后对照和 F3 菜单。它们用于视觉 QA，
截图不参与语义提取。依赖现有 recording 镜像中的 pyte、xterm、Xvfb 和 ffmpeg。

在已有网页测试容器内准备 `/old-gui2tui`、`/new-gui2tui` 和 `/tmp/web-session.json` 后运行：

```sh
python3 /web-tests/compare_tui.py --output /tmp/my-comparison
```

脚本只打开 TUI 的 F3 菜单，不调用 GUI 操作；输出目录必须不存在。
[第一版实景对照与验证记录](../../../docs/validation/readability-20261017/README.md)

支持 `--columns` / `--rows`、`--query`、`--details-pages` 和终端
`--background` / `--foreground`。除主界面对照，还保存 F3、搜索和详情滚动的原始 PTY
与实际终端截图。详情翻页仅发送给新版本的 TUI，不执行 GUI 操作。

`validate_tui_edit.py` 可在已有测试会话中验证一个初始为空的公开文本字段：

```sh
python3 /web-tests/validate_tui_edit.py --field Username \
  --value g2t-aesthetics-verified --output /tmp/my-edit-validation
```

它观察菜单中的确切 `Edit text` 目标再进入编辑，先验证取消不改变 GUI，再用 Ctrl+S
执行 CommitOnly，并经独立 AT-SPI oracle 检查请求文本。它会实际修改测试字段，
不执行 Submit；字段名和值只是验证输入，不是生产匹配规则。
