# 本地资源交付首轮验证

## 最新集中验收

[最终 SSH 结果](final/ssh-results.json)通过：实际 handler 接受、取消、远端真实 F4
经 SSH 至本地处理器、拒绝时零 payload、64 MiB 长度/hash 校验、发送 1 MiB 后
中断隧道清理部分文件、重连后新发送、断线后发送失败。64 MiB 用合成载荷及记录型
handler 验证传输，不伪装成合法大 PDF 或查看器渲染测试。中断由独立协议发送端
在已授权且发送前缀后触发，部分文件清理通过接收端私有目录与基线对比确认。

[阅读器加载证据](final/viewer/result.json)：F4 交付给真实 Okular，公共树出现
接收端生成的 artifact PDF 文件窗口及页码 1。证明文件加载，不证明像素渲染正确；
PNG/WAV 仍只有系统打开器接受证据，未确认显示/播放效果。

F4 显示准备、等待授权、发送字节/总字节和等待校验及 handler；最终结果取自接收端。
工程集中检查：373 库测试、2 inspector、5 CLI 通过，clippy all-targets 通过。
[manifest](final/manifest.json)记录本轮二进制及原始证据 hash。
原始记录继续保留在下面；与最新结论冲突的未完成描述仅代表旧轮次。

- [同机结果](same-host.json)：PNG 99 字节、PDF 333 字节、WAV 1644 字节，均经 broker
  校验后由 /usr/bin/open 接受。没有宣称视觉内容或播放效果已验证。
- [SSH 结果](ssh-results.json)：Linux 容器发送 PDF 至 macOS broker，333 字节交付；
  取消返回失败且 payload_sent=0、artifact_bytes=0。
- [同机驱动](same-host-check.py)、[SSH 驱动](ssh-check.py)、[镜像](Dockerfile.ssh)。
  SSH 使用临时客户端密钥及已固定的临时主机密钥；临时容器和隧道在退出时回收。

首次 SSH 驱动对预期取消使用 check=True，因退出码 1 提前退出；改为保存该结果后
重跑通过。生产代码没有为此修改。测试输出为 handler accepted，不是远端任务成功。
查看器可能仍显示测试文件窗口，测试不会终止用户查看器进程。

当前只完成首轮传输及本地启动，不代表 F4 集成、所有格式或 SSH 故障矩阵完成。

后续 F4 实现：[真实 PTY 结果](f4/result.json)验证 Linux Mousepad 会话打开 F4、
显式输入 PDF 路径、传给记录型 endpoint 并显示处理器接受结果。此测试不启动查看器，
真实系统打开器交付由上方独立测试验证。39 项模态测试、1 项 F4 呈现测试、
cargo check/clippy 通过。当前 SSH 结果使用更新后二进制重跑；未完成断线/大文件矩阵。

SSH 追加验证通过：授权拒绝 payload_sent=0；隧道断开后发送明确失败。
还发现并修复启动器生命周期问题：gui2tui endpoint 原先等待子进程，单独终止
启动器会留下 broker。现在 Unix companion 使用 exec，保留 PID 和信号归属；
SSH 驱动通过统一入口终止、重启 broker 验证 socket 可重新使用。
准备阶段取消及等待接收端响应取消已支持；写 socket 的阻塞仍受 5 秒写超时约束。
传输中途断线、大文件端到端及逐字节 UI 进度仍未完成。
