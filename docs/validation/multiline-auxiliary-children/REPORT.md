# Multiline auxiliary children and delivery — 2026-10-07

FeatherPad 的四个直接子对象都是 filler；两个为空，另两个各有一个 scrollbar。
因此父编辑区不是叶节点，但这些子对象不是文档内容。生产修复允许有界的
Filler/Panel/ScrollBar 子树，拒绝带 Text、EditableText、Document、Hypertext
接口的后代。仍保留完整字符数、格式、写前冲突、后端接受和精确文本读回检查。

真实 GUI2TUI PTY 复测后，编辑仍被下一层格式检查阻止：GetDefaultAttributes
返回空，GetAttributes(0) 返回 size:9、vertical-align:baseline、justification:left、
style:normal、family-name:Monospace，区间 0..16。不能证明这些属性只是默认
样式，因此没有移除纯文本格式保护，未声称 FeatherPad 编辑/reload 成功。

Mousepad 的日志确认目标交付均返回 accepted=true：Line Numbers 的 locator
末尾为 /124，Word Wrap 为 /129；分别按其公开 click 动作精确匹配后调用，
返回 index 0 是匹配所得，不是猜测默认动作。两项后续 CHECKED 断言仍失败。
因此执行链路已经接受，任务效果未确认；不能称为未执行，也不能称为任务完成。

增加 contents-free 产品日志记录 locator、action index、accepted；测试启用
现有 --log-level debug，保存 product.log，并分别提取 backend_action_deliveries。
日志不记录文本载荷。全部 GUI 写操作仍经过 GUI2TUI。

验证：Linux binary 构建、针对性辅助子对象回归测试、Clippy all-targets -D warnings、
fmt check、Python contract 2 项、Python 语法检查通过。没有重跑完整 20 项；
本轮不能更新任务成功总数。

- [交付证据](delivery-evidence.tar.gz)
- [格式属性证据](format-evidence.tar.gz)
