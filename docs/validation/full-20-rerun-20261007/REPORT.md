# 完整 20 项 GUI2TUI PTY 复跑 — 2026-10-07

当前源码 ebdb4053216981b9e9f813428cc031ed4857efe5 重新构建后运行原 20 项
开发任务，结果 **18/20**，相比前次完整运行 17/20 增加 FeatherPad Reload 一项。
原有通过项没有退步。

| 应用 | 通过 | 未通过 |
|---|---:|---|
| Firefox | 5/5 | 无 |
| Mousepad | 3/5 | Line Numbers、Word Wrap |
| FeatherPad | 5/5 | 无 |
| Okular | 5/5 | 无 |

## 执行方式

真实 Rust GUI2TUI PTY，flat 布局；每任务新容器，禁用网络，同时最多两个任务。
Python 只向终端输入，并使用公开 Accessibility 只读观察作断言。
本轮不修改任务、断言或生产代码，不通过替换任务提高通过率。

    python3 tests/research/benchmark/run_scenarios.py       --image gui2tui-capability-tests:local       --binary target/autonomous-linux/debug/gui2tui       --output docs/validation/full-20-rerun-20261007

二进制 SHA-256：
5ff2ce7693428c172c17e1fde7aa493ee5817b9e454b220fb0af258c232a7eaa

镜像：
sha256:5b57dfc2b131aec54dd95f76ecd58ce4cfe751694ab82ec859a1a76b8897ecda

## 两项未通过的具体含义

Mousepad Line Numbers、Word Wrap 都经 GUI2TUI 打开父菜单并投递目标 action。
日志中的目标分别是 /org/a11y/atspi/accessible/124 和 /129，backend
accepted=true。前后公开对象仍为 menu item，均未提供 CHECKED 状态变化，
因此“精确目标 checked 状态改变”的任务断言失败。

这是调用已接受、任务效果未确认，不能将它直接说成调用失败，也不能说任务成功。
本次观测不足以区分 provider 状态未暴露与应用动作未生效。
完整调用日志和前后状态分别保留在两项任务目录。

FeatherPad Reload 现可先经 GUI2TUI 写入未保存文本，再重新加载并处理
Discard changes，最后读回原始内容。本轮完整任务组确认该修复生效。

## 证据和边界

- [summary.json](summary.json)：20 项原始汇总，包含脚本与二进制哈希。
- [comparison.json](comparison.json)：与前次完整 17/20 的逐项差异。
- 每项目录保留 result.json、终端记录、product.log 和容器输出。
- 基础设施失败 0，超时 0，任务断言失败 2。
- 未独立测量 wrong-target、unsafe 或 oracle leakage，不把未测量值报告为零。
- Python 驱动约束测试 2/2 通过；本轮没有生产代码变更，无需重复完整 Rust 测试。
- 本次覆盖 flat 布局原 20 项，不等于全部 GUI 能力、spatial 布局或 F3 专项覆盖。
  F3 专项证据见此前公开操作验证。
- 这是 development suite；未运行 held-out/headline，也未冻结 benchmark。
