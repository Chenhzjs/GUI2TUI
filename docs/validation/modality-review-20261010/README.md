# 文件协作审查修复（2026-10-10）

基于 39995927be8e9e9c2284c8fb55d867237cc7cd57 的后续工作区修复。

- 接收端补齐 GIF/WebP/WebM/OGG/WAV/OBJ 临时文件扩展名，保持 MIME 与本地路径分发一致。
- F4 文件路径编辑纳入文本输入状态，`?` 不再打开帮助；支持 Delete。
- 路径按终端单元宽度滚动，显示插入光标，支持长路径及宽字符。
- 传输进度由应用任务持有；Esc 返回后重新打开 F4 恢复进度，不在活动传输中重新协商端点，避免能力查询失败误取消传输。取消请求同步显示，完成或应用退出时清理进度。

验证：`cargo test --lib modality --quiet`，44 passed；
`cargo clippy --all-targets -- -D warnings` 通过；`git diff --check` 通过。
新增定向回归覆盖 MIME 扩展名、长 Unicode 路径窗口及面板重建后的共享进度显示。
本轮未重新运行真实 SSH/GUI 场景或双架构发布门禁；不将单元测试视为本地查看器实际显示内容的证据。

## 仍未定位的历史失败

检查 closeout-20261010-narrow 与 closeout-20261010-mousepad-recheck 的原始结果：
相同 binary SHA256 下，窄终端首次运行已通过 Chinese multiline edit 的公开读回，
随后 scenario.py:546 的 `terminal.command('Find')` 在等待 Command palette 时超时。
独立复测通过。现存记录不足以区分输入交接时序、事件处理延迟或测试驱动问题，
因此保留未解决状态，不修改历史失败结果，也不加入盲目重试或应用特例。
