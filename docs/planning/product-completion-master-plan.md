# GUI2TUI 产品完整使用主计划

日期：2026-10-08

这是当前产品未完成工作的唯一主计划。后续按阶段顺序执行；每阶段达到退出条件后再进入下一阶段。目标是在选定真实应用和工作流中，只通过 GUI2TUI 完成连续任务，并对 Accessibility 未公开的能力保持诚实边界。

## 目标与边界

2026-10-09 新增用户授权方向：[通用本地资源协作](local-resource-collaboration.md)。
不限图片/PDF，按资源能力将适合外部处理的内容交给本地应用，补资源获取与传输。
该新增范围尚未实现完成；原有语义工作流结论与新增资源协作验收分别记录。

目标包括：理解当前内容和状态；发现并执行公开操作；输入参数并权威读回；处理弹窗、保存、取消、丢弃、重开和动态对象；在刷新、标签切换和终端缩放后继续工作。

不实现 OCR、截图识别、坐标点击、DOM/CDP、UNO、应用私有 API、任意 AT-SPI RPC 控制台、盲按键序列或未经 Accessibility 公开的功能。provider 未公开的正文、画布语义和效果必须记录为边界。

## 阶段顺序

### 收尾检查点（2026-10-08）

最新补充：Line Numbers 已获得公共偏好状态的独立会话对照证据：全新容器的
Show line numbers 为 unchecked，行号菜单操作后的全新容器为 checked。
因此原 20 项均已有任务效果证据（18 项完整回归 + Word Wrap 定向验证 + 行号
独立会话对照），不称为新完整回归 20/20，也不称为行号同会话前后验证。
Okular 追加“Select All Text on Current Page”公开操作后仍未公开正文或选区，
读取无错误；正文阅读/搜索命中仍是未完成的 provider 边界，不能宣称全部完成。

后续复核：同一 binary 的 Word Wrap 定向测试通过，公共 Text 默认属性从
wrap-mode:none 变为 word_char。已修正空文档默认属性采集及效果断言，保留历史
CHECKED 失败；原完整 flat 18/20 加本次定向证据为 19/20。剩余 Line Numbers
及 Okular 正文/搜索命中未确认。Okular 的两次完整树未公开 Document/Hypertext，
未发现正文在 Reader 中遗漏的证据。生产代码和 binary 未改动。

状态：**所选连续工作流完成；整体部分完成并受公共语义边界阻塞**。
完整结论、首次失败、hash、性能原始值与剩余边界见
[最终验证报告](../validation/component-extraction-20261008/REPORT.md)。

- A：基线 commit、source patch、最终 binary、runner、fixture、镜像及应用版本均已记录；首次失败保留。
- B：F3 单行/多行编辑不再要求独立 scene 元素，使用 exact locator、scope、generation/ticket；同名对象分别编辑通过。
- C：全文/单行替换、选区读取、Unicode 范围替换/插入/删除、Value 精确设值和越界未改变、多选成员增删、动态对象出现/调用/删除通过。范围替换不是原子事务，部分失败明确报告。
- D：最终 binary 的 Mousepad、Firefox、fixture 在 120×40 和 80×24 全部通过。
- E：Okular 打开/翻页/缩放/侧栏通过；正文及搜索命中仍未确认。静止两次读取无错误，两个表头 locator 重建；不以关键词输入框作为搜索命中。
- F：每组 60 次导航，关键流程 p95 均低于 60ms；暂停 provider 的显式刷新取消、后续操作、终端挂起/恢复/缩放、外部编辑器交接及退出恢复通过。取消限快照获取阶段，不泛化为任意写操作撤销。
- G：工程检查通过（372 库、2 inspector、5 CLI、2 benchmark）；最终 flat 原 20 项保持 18/20，两项 Mousepad CHECKED 效果仍未确认；FeatherPad 5/5。用户文档和最终报告已更新。
- 最终 binary SHA-256：`5e21c52e1c746816ce8e8a7566fe2d946452472fde00d517b0a813251a1ff5e9`。
- 无新 release/tag。已补测 fixture Action 接口调用至接受返回的投递耗时（14 次，p95 4.89ms）；长时 soak、所有慢读取消路径和更广平台覆盖保留为测量边界。

本轮补充完成：非空选区默认范围替换；Value/文本外部修改冲突拒绝；旧 F3 条目遇到同名重建对象时拒绝投递；归一化 37→35；删除成功但插入失败的部分结果和提示；展开/折叠；Enter 打开 modal、命令退出后直接 Enter 重开、F3 关闭。扩展 fixture 共 33 个检查点通过。后台刷新覆盖操作结果提示的问题已修复。AT-SPI transport 中断/恢复 8 项检查及终端生命周期检查通过。

验收范围说明：Mousepad 连续菜单操作通过，但未单独强制制造该应用的 stale 菜单故障；身份替换拒绝由真实公共 fixture 和既有定向测试证明。虚拟化覆盖限现有 PartialRealized/集合测试及所选流程，不代表任意虚拟列表实测。以上是证据覆盖限制，不宣称已做应用专属故障注入或全平台认证。

A 证据基线 → B 动态身份与刷新 → C 参数化操作 → D 连续工作流 → E 文档阅读边界 → F spatial/窄终端/性能 → G 最终回归收尾。

## A：证据基线

固定源码 commit、工作区 patch、binary、runner、镜像 hash。保留原 20 项 flat 结果。新结果使用唯一目录，记录布局、终端尺寸、应用版本、首次失败、修复后结果、backend accepted 与 task completed 的区别。测试 oracle 每次重新枚举对象，不持有旧 proxy。

退出条件：每个结果可追溯到唯一 binary 和 runner；失败能区分产品、驱动、provider 边界和基础设施。

## B：动态身份、刷新和 F3

重点：src/backend、src/semantic、src/tui/app.rs、src/runtime。

修复对象过期、菜单重建和 popup 替换：区分 stale、transport、permission 和字段读取错误；locator 是后端身份；旧 F3 条目不可调用；刷新后重新建立 scene binding；焦点对象消失时恢复当前 scope；不得靠固定延迟或无限重试掩盖 stale。

统一 F3 链路：当前 session/generation/scope → exact locator → fresh node → capability/action → ticket → backend → authoritative readback。Action、Selection、Value、完整文本写入不额外要求 scene binding；只有 native text/key 才要求焦点。

退出条件：Mousepad 菜单 stale 可分类恢复；同一容器多个文本对象可分别编辑；旧条目不会调用新对象；F3、命令面板和 Enter 规则一致。

## C：参数化操作

只补真实工作流需要的操作：完整多行替换、单行替换、选区读取/替换、插入、删除、Value 精确设值、Selection 成员增删和必要的动态/虚拟化操作。

每个操作必须明确偏移单位、范围和参数，写前冲突检查、写后权威读回，并经过同一 authority 链。child index 与 selected-child index 不混用。Value 要报告范围、归一化、边界和未改变。动态展开、折叠、删除后，新对象进入导航，旧对象拒绝操作。

退出条件：每项都有明确 TUI 入口和定向回归，不需要 locator、快照 ID 或盲按键。

## D：连续工作流

同一应用会话完成，不 reset。

Mousepad：新建 → 中文多行输入 → 查找 → GUI 保存 → 再编辑 → 取消关闭 → 丢弃关闭 → GUI 重开 → 读回保存内容。

Firefox：本地网址 → 页面控件填写 → 提交 → 读取新内容 → 新建/切换/关闭标签 → 回原页面继续操作。

通用 fixture：同名文本对象、多选、Value、动态控件增加/调用/删除、删除后继续操作。

覆盖保存对话框、文件选择、popup、标签切换、外部编辑器返回、目标消失和 transport refresh。中文输入失败若是 provider 或 native strategy 边界，单独记录。

退出条件：最终 binary 上 Mousepad、Firefox、fixture 通过；modal 退出后无需手工重新搜索。

## E：文档阅读

只读调查 Okular 的 frame、document、text、hypertext、page、search、sidebar、outline、zoom 和读取错误；静止场景连续读取两次。区分 cache 丢失、Reader 投影丢失、正文未公开和读取不稳定。

退出条件：打开、查找、下一页、缩放、侧栏、继续阅读逐项有通过、失败或公共边界证据。按钮调用不等于正文阅读成功。

## F：呈现、窄终端和性能

spatial 是默认验收；flat 保持原 20 项。关键操作在 120×40 和 80×24 可达；compact 区域不能隐藏唯一入口；PartialRealized 必须提示部分加载。

记录首屏、缓存导航、投递、刷新、长操作进行中状态、取消和终端恢复。缓存导航 p95 工程目标低于 100ms；真实应用等待单独报告。验证正常退出、外部编辑器返回、SIGINT/unwind、suspend/resume 后 canonical/echo 恢复。

## G：最终回归

必须运行：cargo fmt check、cargo check、cargo test、cargo clippy、benchmark unittest、check-docs、git diff --check；然后原 20 项 flat、Mousepad、Firefox、fixture、Okular、120×40、80×24 和必要的 FeatherPad 复测。

最终报告必须包含逐工作流结果、所有 hash、首次失败和修复结果、accepted/task completed 区别、stale/预算/读取失败/未实现/未公开分类、性能原始值和 p95、布局与终端覆盖、未测量项和剩余阻塞。

## 完成判定

Mousepad、Firefox、fixture 在最终代码和 binary 通过；Okular 每步有结果或公共边界；120×40 关键流程通过；80×24 关键目标和操作可达；过期对象不会误调用；写操作有接受结果和必要读回；工程检查全通过；最终报告和用户文档完成。

若 Okular 正文确实未公开，可以以“部分完成并受公共语义边界阻塞”结束，不能称为完美等价。其他属于 GUI2TUI 丢失能力、呈现遗漏或操作缺口的问题必须继续修复。

## 新上下文启动

读取 AGENTS.md 和本文件；读取最新 validation summary；执行 git status、git log、git diff --check；复现最近失败；做最小修复；跑直接受影响测试；更新证据；满足退出条件后再进下一阶段。不得重置工作区、改历史、发布或启动无关研究。
