# 产品连续使用验证（2026-10-08）

结论：所选 Mousepad、Firefox、fixture 连续工作流已完成，120×40 与 80×24 均通过。
Okular 打开、下一页、缩放、侧栏通过，搜索结果和继续阅读正文未确认；本轮以
**部分完成并受公共语义边界阻塞**收尾，不代表任意 GUI 功能等价。

## 身份与复现

最新行号补证：[独立会话对照](boundary-lines-baseline/comparison.json)记录两个新容器
的公共 Show line numbers 复选框：未执行菜单时 unchecked，执行菜单后 checked。
写入全部经 GUI2TUI，测试只读取公开树。行号设置效果因此获得证据；原 20 项
累计都有任务效果证据，但历史完整回归仍为 18/20，不能改写成新回归 20/20。
同会话对照尝试保留在 boundary-lines-verified / boundary-lines-modeless：偏好窗口
没有公开 Close 操作，打开后主窗口目标不在可用作用域，因此没有猜按键或绕过作用域。
干净基线在 boundary-lines-baseline，菜单后状态在 boundary-preferences。

Okular [选择当前页全部文本补测](boundary-selection/result.json)仍为公共内容未确认：
公开操作之后 396 个对象、9 个 Text 输入控件，无正文和选区、无读取错误。
这条公开操作也没有补出阅读能力。正文和搜索命中仍未完成；不通过剪贴板、文件解析
或私有接口替代原任务。该轮只增加证据，生产代码和二进制不变。

后续边界复核（同一产品 binary）：Mousepad Word Wrap 的公共 Text 默认属性由
wrap-mode:none 变成 wrap-mode:word_char，实际效果已确认。旧 oracle 只采集非空
多行文本的默认属性，遗漏了该空文档；现已补采并用同一 locator 的属性变化断言。
[初次补读](boundary-attributes/mousepad-word_wrap/result.json)保留旧 CHECKED 失败；
[定向回归](boundary-wrap-verified/mousepad-word_wrap/result.json)验证新断言。
历史完整 flat 仍为 18/20；合并本次同 binary 定向证据，任务覆盖为 **19/20**，
不是一次新跑的完整 20 项。Line Numbers 的公开状态、关系、子项和文本默认属性
仍不足以确认效果，不从几何变化猜测开关状态。

[Okular 接口审计](boundary-attributes/okular-interface-audit.json)记录原始证据 hash：
两次 396 对象树均没有 Document/Hypertext，9 个 Text 均同时公开 EditableText，
没有 PDF 正文。当前证据未发现正文在 Reader 中遗漏；结论仅限该版本和场景。

- 源码基线：`ebdb4053216981b9e9f813428cc031ed4857efe5` 加工作区补丁，不是新的 release/tag。
- 最终 Linux binary SHA-256：`5e21c52e1c746816ce8e8a7566fe2d946452472fde00d517b0a813251a1ff5e9`。
- 镜像：`sha256:5b57dfc2b131aec54dd95f76ecd58ce4cfe751694ab82ec859a1a76b8897ecda`。
- Linux amd64，arm64 主机仿真；私有 D-Bus、Xvfb、真实 GUI2TUI PTY，容器禁网。
- [当前 hash 清单](completion-audit/manifest.json)记录源码 patch、binary、runner、fixture、生命周期脚本；
  每个 run 的 summary.json 保留该次 runner/script hash。测试修正未覆盖首次结果。
- [应用版本](qualification/application-versions.txt)：Mousepad 0.5.10、Firefox ESR 153.4、
  Okular 22.12.3、FeatherPad 1.3.5，GTK 3.24、Qt 5.15。

复现使用 tests/research/benchmark/run_scenarios.py，参数见各目录的 run_scenarios.py
及 summary.json；`--workflows --layout spatial --columns 120 --rows 40` 为正常宽度，
`--columns 80 --rows 24` 为窄终端；不传 `--workflows` 为原 20 项。

## 工作流与证据

| 工作流 | 120×40 | 80×24 | 证据目录 |
|---|---|---|---|
| Mousepad 新建、中文多行、查找、保存、再编辑、取消关闭、丢弃、重开读回 | 通过 | 通过 | audit-final-spatial / audit-final-narrow |
| Firefox 本地页、填写提交、标签新建/切换/关闭、返回继续提交 | 通过 | 通过 | audit-final-spatial / audit-final-narrow |
| fixture 扩展 33 项：同名编辑、选区/范围、多选、Value、冲突、旧条目、modal | 通过 | 通过 | audit-feedback-v2 / audit-final-narrow |
| Okular 打开、下一页、缩放、侧栏 | 通过 | 未单独测量 | audit-final-spatial |
| Okular 搜索结果、继续阅读正文 | 公共内容未确认 | 未单独测量 | audit-final-spatial |
| 原 20 项 flat（320×50），含 FeatherPad 全部五项 | 18/20 | 不适用 | audit-final-flat |

原 20 项的 Mousepad Line Numbers / Word Wrap 仍为后端 accepted=true，公开 CHECKED
变化未确认。它们既不计成功，也不计确定投递失败；其余 18 项通过。

Okular 静止状态两次读取均为 396 个对象，公共读取错误为零。两次树只有两个
table column header 的 locator 被 provider 重建；共有 locator 的字段一致，
正文 Alpha/Beta 未成为可读文档 Text。按钮调用、搜索框中的关键词都不作为正文
或搜索命中证据。没有用 OCR、PDF 解析、截图或私有接口补出正文。

## 实现与参数语义

- F3 单行/多行编辑以 exact locator 和当前 scope 为依据，解除对独立 scene 元素的依赖；
  独立输入浮层保证编辑缓冲区可见。单行提交前比对原值，多行保留原有冲突与读回。
- Set value 输入精确有限数值，展示范围，提交检查原值；读回区分归一化、未改变和确认。
  fixture 验证 37、超范围 101 保持 37、随后按公开步长增加。
- Read text selection 返回公开选区。Edit range 使用 `start:end:text`，偏移为 Unicode
  字符、end 不包含在范围内；空范围插入，空内容删除。InsertText 长度是 UTF-8 字节。
  中文替换/插入/删除均读回通过。删除后先确认剩余全文再插入，部分失败不宣称原子回滚。
- 显式 r 刷新的快照获取阶段支持 Esc；暂停真实 provider 后取消，再恢复并执行操作通过。
  不是所有后台读取/写操作的通用取消，也不撤销已投递写入。
- suspend/reattach 创建 EventStream 前销毁旧 reader，避免旧读取线程占用共享锁。

## 生命周期与工程检查

[当前生命周期日志](completion-audit/lifecycle-final.log)验证正常退出、
SIGINT、SIGTERM、受控 unwind、suspend/resume、四次外部编辑器交接、子进程回收、
旧 generation 拒绝、冲突保留、挂起 handler 的重复信号退出、私有临时文件和有界清理。
测试驱动修正为按公开命令名称选择，并重建 ANSI 画面，避免固定 Tab 次数和原始字节
连续匹配。所有最终工作流退出均确认 canonical/echo 恢复。

工程检查通过：cargo fmt --check、cargo check、cargo test（372 库、2 inspector、
5 CLI）、cargo clippy --all-targets -- -D warnings、benchmark unittest（2）、
python3 scripts/check-docs.py、git diff --check。当前 cargo test/clippy 日志在 completion-audit/，上一轮完整检查日志保留在 qualification/。最初误用不存在的
check-docs.sh 已改用仓库实际的 Python 脚本完成检查。

## 性能和测量边界

[当前原始测量](completion-audit/performance.json)保存最终流程的首屏、导航样本和步骤耗时；
[fixture 细项](completion-audit/fixture-metrics.json)包含投递及取消耗时。
每组 60 次导航，p95 使用 nearest-rank，包含 PTY 和轮询。
fixture 现在在缩放之前采样并记录实际终端尺寸，因此 120×40 有独立证据。

| 流程 | 首屏 ms（120×40） | 导航 p95 ms（120×40） | 导航 p95 ms（80×24） |
|---|---:|---:|---:|
| Mousepad | 1009 | 32.77 | 27.72 |
| Firefox | 832 | 20.83 | 59.36 |
| fixture | 486 | 31.07 | 35.86 |
| Okular | 1889 | 35.03 | 未测 |

fixture 的 14 次 Action 接口调用至接受返回耗时 p95 4.89ms，不含前置身份核验、
后续刷新及任务效果确认，也不称为全链路延迟。阻塞刷新取消耗时 357ms。
这些是本环境工程测量，不能推广到原生机器或长期性能保证；长时资源 soak、
所有慢读路径取消和全平台性能未测。上一轮测量保留在 qualification/performance.json。

## 补充验收与修复

当前 fixture 的 33 项检查全部通过：非空中文选区读取和默认选区替换；第二个
GUI2TUI 会话修改原值后的 Value/文本冲突拒绝；持有旧 F3 条目、删除并重建同名
对象后拒绝旧操作且新入口可用；37→35 归一化；删除成功、插入被 provider 拒绝后
真实残余文本读回；展开/折叠；Enter 打开 modal、命令关闭后无需搜索直接 Enter
重开、F3 关闭。两个尺寸均执行该流程。

新发现并修复的产品问题：Value 归一化及范围编辑部分失败提示会被被动事件刷新覆盖。
现在子树更新及 application dirty 全量刷新均保留这些反馈；最终真实场景验证通过。
初次失败和修复过程保留在 audit-feedback* 目录。驱动还修正了 pyte 的孤立中文
续格显示，未改变被测产品语义。

[transport 恢复日志](completion-audit/transport-final.log)的 8 项检查全部通过：
隔离测试容器内中断 AT-SPI 总线、长时间不可用、显式恢复、拒绝旧绑定、恢复后新
操作可用、旧操作不会复活、新 generation 重建、transport 恢复不等于语义重新授权。
补充镜像及 Dockerfile 记录在 completion-audit/manifest.json；仅安装公开测试所需的
PyQt6/x11-utils。首次缺依赖及 ANSI 原始字节断言失败保留在 recovery*.log，
最终驱动等待新 generation 就绪，不依赖连续 ANSI 文本。生命周期重新验证全部通过。

## 首次失败与修复分类

| 首次证据 | 分类 | 修复/最终证据 |
|---|---|---|
| unified-current 的文件选择器定位失败 | 驱动过滤过宽，入口实际存在 | 增加公开角色/操作名过滤，qualified-mousepad-rendered 通过 |
| continuation-current fixture 无法启动 | 基础设施：旧镜像缺 GTK typelib | 使用已验证镜像，continuation-qualified-image 通过 |
| cancellation-current 恢复失败 | 产品 reader 交接缺口 | 显式 drop 旧 EventStream，resume-reader-fix 及最终 fixture 通过 |
| lifecycle-current 多次旧驱动失败 | 固定 Tab 定位、ANSI 局部重绘匹配 | named command + rendered frame，qualified-rendered 全通过 |
| range-current 中文替换只发生删除 | 产品 InsertText 长度单位错误 | UTF-8 字节长度，range-utf8-fix 及最终 fixture 通过 |
| range-current / range-utf8-fix 选区断言 | 驱动宽过滤及历史选区假设 | 精确入口并记录当前 public selection，qualified-mousepad-rendered 通过 |
| qualified-spatial / qualified-selection-read Mousepad 空错误 | 驱动 StopIteration，读取半帧 | 等待完整 palette frame，保留错误类型与 traceback，最终通过 |

stale 仍按 exact locator/generation 拒绝；读 oracle 的失效快照最多整次重取三次，
不重放写操作。预算截断保留 PartialRealized，不等同 provider 未公开。参数操作
失败、公共内容未公开、投递接受但效果未确认分别报告，不混为成功。

## 剩余边界

[Okular 公共路径及源码核查](okular-public-path-audit/README.md)：31 条公共关系的
15 个目标全部已在子树中；v22.12.3 源码未找到 QAccessible 实现，PageView 自绘，
文字选择走内部 TextPage，Accessibility 绘制标志用于颜色转换。证据支持当前
版本正文没有接入 AT-SPI，而非 Reader 漏投影；不推广到所有版本或全部焦点状态。

Okular 正文/搜索命中仍无法由当前公开语义确认，阻塞文档完整阅读。
Mousepad Line Numbers 已由上方独立会话的偏好复选框状态补证。选区可能随 GUI
状态变化清空，读取 none 不等于读取失败。范围输入只提供单行参数，完整多行编辑
继续使用外部编辑器。动态/虚拟化覆盖限于本工作流和已有定向测试，不宣称任意应用覆盖。Mousepad 菜单连续流程通过，但没有单独强制注入该应用的 stale 菜单；旧条目拒绝由公共 fixture 真实对象替换及既有定向测试证明。
未发布、未改 tag、未重写历史。
