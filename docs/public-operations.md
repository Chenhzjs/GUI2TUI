# 操作公开的 Accessibility 能力

本文对应当前开发分支，尚未作为新版本发布。GUI2TUI 将应用公开的操作映射为
终端入口，由用户决定执行什么。它不要求提前知道 action 的效果；后端接受
调用与用户任务完成是两件事。

## 选择并调用 action

1. 在应用场景按 **F3**，打开公开操作列表。
2. 输入目标名称过滤条目。
3. 用上下方向键选择正确目标及操作。条目包含名称、角色、快照 ID 和 action 名。
4. 按 Enter 调用，查看后端反馈和更新后的场景。

例如同一对象公开 click 和 show-menu 时，两项分别列出。选中 click 就按精确
名称调用 click，不猜索引零，也不把其他名字强行解释成点击。名称重复而无法
唯一定位的 action 会报告不可用。快照 ID 用于区分当前场景对象，不是跨重启的
稳定标识。模态窗口存在时，入口遵循当前模态作用域。

原有 Enter 是按控件语义映射的快捷操作，冒号是语义命令面板；**F3** 是补充的
公开操作入口。**F4** 仍是资源入口。Esc 关闭列表，r 刷新场景。

## 多选与数值

当父对象公开 Selection 接口时，F3 列出子项的 Add to selection 和
Remove from selection。它们调用成员增删接口，不主动先清空全部选择。
最终选择行为由应用决定；单选容器仍可能替换原选择。

具备可调整 Value 的对象提供 Value increase/decrease，使用应用公开的步长和
范围。反馈显示原值、请求值和实际读回值；达到边界时不冒称数值发生变化。
明确只读值及缺少有效范围、步长的对象不会被伪装成可调整控件。

## 文本输入与全文替换

单行 EditableText 控件使用 Enter 进入编辑、Ctrl-S 提交。浏览器地址栏提交
文本后，可显式使用 Alt-Enter 向经过身份及焦点检查的当前控件发送原始 Enter。
这不是自动推断的 Submit，也不能保证所有 provider 都支持该路径。

对公开完整多行 Text/EditableText 的文档，配置本地编辑器后，在文档入口按 e：

    [interaction.complex_text]
    program = "/usr/bin/vim"
    args = ["{file}"]

配置文件位置见 [入门指南](getting-started.md#optional-complex-text-handler)。
编辑器修改 GUI2TUI 的临时文本副本，退出后由 GUI2TUI 经应用接口提交。应用的
实际文件不作为写入后门。明确全文替换允许 provider 替换格式或嵌入内容，
子对象存在不构成拒绝理由。密码不导出；不完整文本不冒充完整全文。

## 资源预算和排查

    gui2tui --session desktop --app NAME --max-nodes 20000 --max-depth 96 --max-edit-bytes 1048576

默认树预算是 10000 个对象、64 层，全文编辑预算为 256 KiB。增大预算后重新启动
GUI2TUI。这些是资源边界，不是危险程度分类。F3 对已记录的树截断显示 partial
提示；提高预算不能使应用尚未公开的虚拟化对象凭空出现。

- 没有看到预期操作：先刷新，再查看 F3；确认应用确实公开了目标和接口。
- 目标过期或作用域改变：刷新并重新选择当前目标，不能复用旧对象身份。
- 后端接受但没有看到效果：表示调用已接受，效果仍由应用行为及用户观察判断。
- 文本写入未确认：查看真实读回与错误，不把终端本地显示当作成功。

## 实际演示与验证范围

[操作视频及复现说明](demo/public-operations/README.md) 包括精确 action、多选、
Value、Firefox 地址跳转和 Mousepad 新建编辑。生产实现不按应用名称或品牌
分支；这些应用只用于测试。更多结果见
[公开能力验证报告](validation/public-operations/REPORT.md)。
