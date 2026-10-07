# GUI2TUI PTY task correction — 2026-10-07

本轮纠正了旧 scenario runner 直接使用 pyatspi 操作 GUI 的错误。旧结果不能
证明 GUI2TUI 能完成任务。本轮所有操作来自真实 Rust GUI2TUI 的 PTY，
pyatspi 仅用于测试侧读取。没有运行 held-out/headline benchmark。

## 代码修复

与 v1.1 对比，新增的 `apply_human_capability_projection` 会把内容摘要的
BrowseContent 当作仅适用于 Table 的后端能力，删除文档绑定。修复保留由
公开 content model 构建的 DocumentSummary 绑定；文档写入仍经过原有
外部编辑、operation authority 和文本 readback 链路。新增一条回归测试。
本轮没有宣称完成所有研究遗留代码的清理。

## 固定版本真实运行

4 个应用，各 5 个任务，每任务新容器，network none，flat 布局。
Firefox 地址测试使用本地 data URL，不能据此声称外网访问已经验证。
Mousepad 外部编辑器只修改 GUI2TUI 导出的临时表示，由 GUI2TUI 提交。

| 应用 | 通过 | 输入已发送但完成未确认 | 未通过 |
|---|---|---|---|
| Firefox | 地址输入并跳转 | Reload | New tab、List tabs、Firefox View |
| Mousepad | Find、新建并编辑文字 | Fullscreen | Line Numbers、Word Wrap |
| FeatherPad | Select Text、New、Find | Reload、Side-Pane | 无 |
| Okular | Sidebar、Browse、Highlighter | Zoom In | Underline |

合计 9 passed、5 dispatched_requires_task_assertion、6 未通过；没有容器
启动失败或超时。输入已发送不表示后端已接受，更不表示任务成功。查找测试
只断言打开后聚焦可编辑单行输入，不包括输入查询和验证搜索结果。

Firefox 三项和 Mousepad 两项失败于终端控件查找。Firefox 终端显示部分
按钮 action unavailable；尚未完成 provider/投影/驱动三者的根因区分。
Mousepad 菜单项仍需验证打开父菜单后的可达性。禁止用直接 AT-SPI 写调用
替代这些失败路径。Okular Underline 有两个同名 check box，其中一个确实
改变了 CHECKED；唯一目标断言失败，因此保留失败，不能据此认定后端没执行。

当前执行能力尚不完善，但两个核心端到端任务已经成立。后续瓶颈主要是
入口可达性、同名目标消歧，以及其余任务断言不足；不是缺少 runtime effect
推断。spatial 布局没有在本轮重新验收，不能外推 flat 的结果。

## 证据与检查

- [summary.json](summary.json)：20 项结果、镜像标识、脚本哈希及二进制哈希。
- [evidence.tar.gz](evidence.tar.gz)：固定运行脚本、逐任务 JSON、终端 ANSI/文本、日志。
- [运行说明](../../../tests/research/benchmark/README.md)。

Linux Rust binary 构建通过；cargo fmt check、cargo check all-targets locked
通过；现有 tui::app 14 项测试通过，新增文档绑定回归测试通过；Python
contract 2 项通过，Python 语法编译通过。文档审计与 diff 空白检查通过。
此结果没有提供 wrong-target、unsafe 或 oracle-leakage 的独立测量，不能
把未测量写成零。
