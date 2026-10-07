# 公开操作能力补全 — 2026-10-07

本轮针对能力原则审查的实现缺口。没有新增自动探索、安全词门禁或效果预测。

## 用户入口

- F3 打开公开操作列表，搜索目标名称，用方向键明确选择 action 再 Enter。
  每条展示目标名称、角色、当前快照 ID 和动作名。精确命名调用不猜 action[0]，
  重名动作会被拒绝；作用域、目标身份、generation 和 ticket 仍由正常运行链检查。
- 同一列表提供 Selection 的 Add to selection / Remove from selection。
  按精确子对象定位当前索引，添加不会主动清空其余选择；后端读回成员状态。
- Value increase/decrease 使用公开范围和 increment，返回真实值。Scrollbar
  等具备接口的角色现在可用；明确只读值、无效范围或缺少增量仍如实降级。
- Cache / walk 两种发现路径及 flat / spatial 展示按 EditableText、Value
  能力处理，不再只认少数控件角色。密码、完整文本读取及写入读回约束保留。
- --max-depth、--max-nodes 显式出现在帮助；--max-edit-bytes 可调整全文编辑
  默认 256 KiB 预算。F3 对已记录的树截断显示 partial 提示。预算不是安全判断，
  也不代表无限文档、虚拟化内容或所有 provider 都已支持。

F4 仍是资源入口；Enter 仍是原有语义快捷操作。通用 action 接受投递不代表
应用任务效果已被确认。ReadOnly 内容不再单独隐藏公开命名 action。

## 真实执行证据

fixture/result.json：通过真实 GUI2TUI PTY 完成以下四项，全部通过：

1. 精确选择公开 click，应用标签变化。
2. 添加 Alpha、Beta，二者同时选中。
3. 移除 Alpha，Beta 仍选中。
4. Scrollbar Value 从 20 增加到 25。

app-regressions/summary.json：6/6 通过：Firefox 地址输入并跳转、新标签、刷新；
Mousepad 新建并编辑；FeatherPad 新建、编辑后 Reload 并丢弃更改。
每次应用任务使用全新容器。Python 只向 GUI2TUI PTY 输入，Accessibility 用作
只读 oracle；生产代码没有应用名称、窗口标题、品牌或测试标签分支。

这是一轮针对性回归，不重写此前完整 20 任务结果；之前 Mousepad 两个菜单
操作 accepted 但 CHECKED 未观察到的结论仍保留。未运行 held-out/headline。

证据记录各自二进制 SHA-256。后续收尾修改仅补 Value 文本展示、预算错误提示、
CLI 参数校验和定向回归，最终源码另经 Rust 检查，不将旧证据冒充最终二进制全量运行。

## 代码验证

cargo check --all-targets --locked、Clippy -D warnings、fmt、diff check、
document audit 全部通过。Rust 全目标运行中 370 个库测试、2 个其他集成测试
通过；CLI 的旧“隐藏预算”断言更新后 5/5 定向复测通过。Python 2 项执行路径
contract 通过，覆盖两个驱动脚本禁止直接 Action/EditableText/Selection 写调用。
