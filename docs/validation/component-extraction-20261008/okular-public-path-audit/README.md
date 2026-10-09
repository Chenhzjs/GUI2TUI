# Okular 公共阅读路径核查

范围：Debian 4:22.12.3-1+deb12u1、现有两页 PDF fixture、当前 GUI2TUI binary。
结论：运行时公共树、关系目标和对应版本源码共同支持“该场景的 PDF 正文没有接入
AT-SPI 文本读取接口”，未发现 GUI2TUI 漏掉现成正文入口。不是对所有版本的证明。

## 运行时证据

[result.json](result.json)包含通过 GUI2TUI 打开、搜索、翻页、缩放、选择当前页文字
后的公开树，并枚举每个对象的公共 relation targets。31 条关系、15 个唯一目标，
均已在普通子树中，没有关系指向树外正文。Text 目标仍是输入控件；没有正文选区，
公共读取错误为零。没有额外执行文档区域强制聚焦测试，不宣称遍历所有焦点状态。

## 对应源码

- [PageView 声明](https://github.com/KDE/okular/blob/v22.12.3/part/pageview.h#L53)：
  继承 QAbstractScrollArea，页面不是普通 QTextEdit 文档。
- [内部选中文字](https://github.com/KDE/okular/blob/v22.12.3/part/pageview.cpp#L1011)：
  从内部 Page/TextPage 读取；[复制和全选](https://github.com/KDE/okular/blob/v22.12.3/part/pageview.cpp#L1116)
  分别写剪贴板、更新内部选择区域，没有公开文本接口的实现。
- [页面绘制](https://github.com/KDE/okular/blob/v22.12.3/part/pageview.cpp#L1813)
  在 paintEvent 中绘制；[Accessibility 绘制标志](https://github.com/KDE/okular/blob/v22.12.3/gui/pagepainter.cpp#L234)
  对应 changeColors/renderMode，不是 AT-SPI 正文能力。
- v22.12.3 完整源码检索 QAccessible、qaccessible、installFactory 无匹配。
  Qt 通用控件仍可提供菜单和输入框的 Accessibility；这不意味着自绘正文会自动公开。
- Debian 补丁包唯一列出的 fax-security.patch 修改传真解码，不补充 PDF 可访问接口。

源码仅用于诊断，不接入产品，也未通过内部 API、剪贴板或解析 PDF 代替语义来源。
[audit-summary.json](audit-summary.json)记录源码下载地址、归档 hash 和运行证据 hash。

“打开成功”证明文档加载和界面操作可用，不等于正文已实现 AT-SPI Text。
当前剩余限制应表述为“经当前公共路径及源码核查，未找到该版本的正文可访问实现”，
而非已证明任意模式、版本或未来实现都无法提供正文。
