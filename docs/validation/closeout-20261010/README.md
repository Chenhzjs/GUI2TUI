# 版本收尾验收 2026-10-10

已完成 resources.socket 保存配置和 remote 模式；远端禁用引用/同机路径打开，
使用显式字节传输。SSH 隧道仍由用户启动，不隐式连接主机。

120×40 fixture、Mousepad、Firefox 全通过。80×24 fixture、Firefox 通过；
Mousepad 首次在外部编辑后等待命令面板超时，独立同 binary 复测通过。
首次结果保留，不能称首次全绿，也未证明超时根因。

SSH 远端 F4 验证 remote 模式拒绝同机打开，然后发送到 macOS handler；
64 MiB 完整性、部分中断清理、重连、拒绝和取消通过。
全套 Rust 测试、all-targets clippy、6 项 release assembly 测试通过。
本机 amd64 仿真 RC 包在补齐 GTK4 镜像依赖后包内 fresh-home smoke 通过，
覆盖动作、Value、外部编辑、冲突、失败和终端恢复。

原始证据位于本目录和相邻 closeout-20261010-spatial、closeout-20261010-narrow、
closeout-20261010-mousepad-recheck 目录。失败日志未覆盖。

尚未提交、推送、打 tag 或发布。本机包仅用于验证：dirty 工作区改动不在
BUILD-INFO 的 HEAD 中，打包脚本只包含已跟踪文档，新增未跟踪文档未入包。
不能把该产物作为正式发行包。正式发布仍需审查提交、源码冻结、收齐文档，
原生 Ubuntu 22.04 x86_64/aarch64 CI、包 smoke、ABI、双包 assembly 与 provenance。
旧公开版本和 tag 未变。
