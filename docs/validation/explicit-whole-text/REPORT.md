# Explicit whole-text replacement and capability audit

用户明确选择完整文本替换时，格式或子对象不应成为 GUI2TUI 的拒绝理由。
应用决定 EditableText.SetTextContents 如何替换原内容。本轮移除叶节点、
辅助子树白名单、非空格式属性、Document 接口和 ManagesDescendants 拒绝项，
删除不再适用的辅助子树测试和错误类型，重命名方法去掉 plain 字样。
另移除多行资格检查前八个目标的截断。仍保留明确编辑接口、可编辑多行状态、
密码屏蔽、完整文本长度、写前冲突、session/scope 和写后精确读回。

真实 GUI2TUI PTY 针对性复测：

- FeatherPad：完整替换并读回成功，Reload 后在应用确认框选择 Discard changes，
  恢复原文件文本，通过。没有直接改 backing file。
- FeatherPad New：通过。
- Mousepad New + 编辑：通过。

这是三个针对性任务结果，没有重新跑完整 20 项，不宣称当前版本整轮 18/20。
[summary.json](summary.json) 和 [evidence.tar.gz](evidence.tar.gz) 保存固定脚本、
二进制哈希、终端记录、公开前后状态与后端交付日志。

## 执行链路审查

已检查 backend 文本写入、TUI action resolver、命令入口、capability graph
投影及 palette。没有发现 Human 点击链路按 risk/unknown effect 字段直接
拒绝；研究 risk 分类不能据此当成人类操作门禁。以下缺口仍未完成：

- 普通 Enter 依赖角色兼容的语义 action 映射。其他公开命名 action 尚无
  完整的用户选择入口。应提供明确命名操作，不能把 Enter 猜成第一个 action。
- capability graph 对已有 scene 能力还有重复资格检查，需逐项证明不会
  丢失已暴露能力；不能声称此次已经完成全局能力完整性证明。
- palette 有 DEFAULT_CONTEXT_LIMIT 显示截断，需要进一步验证后续条目的
  搜索/分页可达性，不能用展示预算永久隐藏能力。
- 按角色映射 Value/选择/文本编辑仍有支持范围，属于实现覆盖缺口，不能
  将“当前未支持”包装为操作危险。

项目 AGENTS.md 已记录用户要求：公开能力不能被效果推测否决，交付接受和
任务效果分开报告。上一轮 Mousepad 行号/换行已确认 accepted=true，仍仅
缺效果证据，不能算执行链路失败。

检查：Linux build、Clippy all-targets -D warnings、backend 27 项测试、
fmt check、Python contract、文档和 diff 审计通过。
