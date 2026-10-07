# Public capability presentation audit — 2026-10-07

检查范围：AT-SPI action/text/value/selection，语义 action resolver，命令生成，
capability graph 投影，palette，操作 authority 与后端调用。不是对所有应用
公开接口支持度的完整证明。

## 本轮修正

- palette 浏览和搜索原先只保留前 15 项，无翻页即可永久丢失后续命令。
  删除截断，方向键遍历全部条目，显示从当前选择开始，标题显示位置/总数。
- F2 全应用范围开关原先仅搜索生效；空搜索浏览现在也生效。执行仍使用
  当前 scope 的 allows_node 校验，展示背景命令不授权跨模态操作。
- capability graph 不再清除语义 scene 已解析出的 binding，也不再过滤
  CommandHierarchy 中的合法命令；图谱记录不等于第二套操作 authority。
- action resolver 测试的 dangerous 命名改成 distinct_operations。Delete
  不能被推断为通用 Activate，不等于禁止用户明确选择 Delete。

## 已核查的限制分类

| 检查项 | 判断 |
|---|---|
| unknown effect / risk 分类 | 未发现 Human 点击入口以这些字段直接拒绝；不应扩大为用户操作门禁 |
| session、generation、locator、ticket | 保留，防止操作被发往错误对象 |
| modal scope | 保留，遵从 GUI 当前操作对象约束 |
| 明确接口/动作支持与后端失败 | 保留，如实表达可调用性 |
| whole-text 子对象和格式保护 | 上一提交已删除，属于额外内容保护政策 |
| 文本完整性、写前冲突、精确读回 | 保留，不能把部分读取或拒绝写入说成完整替换 |
| 密码屏蔽 | 保留项目约束，不导出秘密文本 |
| 文本 256 KiB、枚举/超时上限 | 资源边界，仍会限制支持范围，应明示，不能称为危险 |

## 仍有实现缺口

1. 任意公开命名 Action 尚无统一 TUI 选择入口。现有 Enter 是语义快捷操作，
   不应猜 action[0] 或把不同动作当 Activate。需新增精确命名入口才能完整
   暴露剩余能力；本轮没有声称已实现。
2. 多选与若干角色的 Value、EditableText 支持范围仍有限。属于尚未实现
   的操作语义，不能以“不安全”作解释。
3. 文本大小、bootstrap 等资源预算仍可能影响能力发现；需与完整性标记
   一起评估，不能通过删除预算假装已验证无限规模。

## 验证

新增 40 个同名命令的可达性回归（浏览/搜索均不截断），补空搜索 F2 测试；
补图谱不删除已解析 binding 的回归。TUI 测试 111 passed，palette 定向复测
通过；all-targets check、Clippy -D warnings、fmt、Python contract 通过。

真实 GUI2TUI PTY 定向复测：Firefox New tab/Reload、Mousepad New+编辑、
FeatherPad New/Reload。证据目录为 capability-principle-audit；未重跑完整
20 项，也不改变此前整轮结果。生产改动没有应用名称/品牌分支。
