# 几分钟开始使用 GUI2TUI

人类 TUI 模式不需要模型或 API 密钥。运行目标是 Linux 公共 Accessibility；
macOS 上能编译不代表能直接操作 Linux GUI。安装和无桌面环境配置见
[Getting started](getting-started.md)。

## 先打开应用

应用已经在当前 Linux 桌面会话运行时：

```sh
gui2tui --session desktop doctor
gui2tui --session desktop
```

方向键选择应用，Enter 进入；列表太长时按 `/` 搜索。列表为空时，先在同一
会话启动应用，再按 `r` 刷新；按 `d` 检查连接。使用 Managed 环境时，先完成
Getting started 的 setup，再用 `--session managed`；不同会话的应用不会出现在一起。

## 先记住五个键

| 按键 | 用法 |
| --- | --- |
| Tab / Shift-Tab | 在当前区域移动高亮控件。 |
| Enter | 使用控件、进入文本编辑、选择选项，或阅读正文。 |
| F3 | 搜索当前交互作用域中的公开动作；输入名称，方向键选结果，再按 Enter。 |
| Esc | 关闭面板或取消本地编辑；在主界面按 Esc 会退出程序。 |
| F1 | 查看当前面板的帮助；上下键滚动，Esc 返回。 |

Tab 找不到需要的区域时，按 F6 切换区域；同一区域有多个面板时用 Ctrl-Tab。
不必记住所有快捷键：先看高亮位置和底部提示，需要其它动作就用 F3。

第一次可以试着选中文本字段、Enter、输入少量内容，再按 Esc：这会取消本地
编辑，不写回 GUI。确定要应用时，用 Ctrl-S，只应用文本并检查原 GUI 回读。
普通 Enter 还可能请求应用公开支持的提交，因此只想改文本时优先用 Ctrl-S。
编辑状态下 q 和 ? 都是普通文本，帮助用 F1。

正文用 Enter 进入 Reader；`/` 搜索已加载文本，在搜索页按 Ctrl-F 继续搜索公开
正文，Esc 返回。

## 怎样理解提示

- **read only / action unavailable**：当前默认操作不可用；F3 可能仍有其它公开动作。
- **未命名 / no public title**：应用没有提供这个名称。上下文可以辅助辨认，编号和顺序不证明对象用途。
- **not read / read failed / partial**：未读、读取失败或只有部分覆盖，不代表其它内容和动作也不可用。
- **unknown result**：交付或效果尚未确认。先查看当前界面，主界面按 r 重新观察，再决定下一步；不要直接重复可能已生效的动作。

`b` 返回应用选择；`q` 或 Ctrl-C 退出。F4 资源、F7 工作流和外部编辑器属于可选
功能，正常使用不依赖它们。密码输入和视觉验证可能需要通过获准的人工接管操作原 GUI。
