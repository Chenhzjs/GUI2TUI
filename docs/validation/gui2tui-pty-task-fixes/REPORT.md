# GUI2TUI task fixes — 2026-10-07

固定版本真实 PTY 测试从 9/20 提高到 17/20，未换掉失败任务。所有 GUI
写操作由 Rust GUI2TUI 完成。Python 发送终端输入，并只读公开 Accessibility。

| 应用 | 通过 | 未通过 |
|---|---:|---|
| Firefox | 5/5 | 无 |
| Mousepad | 3/5 | Line Numbers、Word Wrap |
| FeatherPad | 4/5 | Reload 编辑前置条件 |
| Okular | 5/5 | 无 |

## 一、错误现象与复现

旧测试存在动作无法识别、长命令裁剪、工具栏命令拒绝、短 effect 观测期限
取消动作，以及缺少任务断言等问题。使用原四个应用、20 项任务、flat 布局、
每任务独立容器复测。运行方法见[测试说明](../../../tests/research/benchmark/README.md)。

## 二、根因与修复

1. Firefox 的公开 GetActions 返回三个字段均为分号的元数据，而 GetName
   返回 press。后端改为按 NActions 枚举 GetName 读取机器动作名。精确匹配
   名字，不猜 action index 0；规则不依赖应用品牌。
2. 有公开 click 的 Menu 此前一律视为容器。现在具有明确动作的 Menu 获得
   操作入口，隐藏菜单项仍要求先打开父菜单。
3. 命令面板按条目数定高度，长路径折行被裁剪。现在给折行预留高度并利用
   宽终端空间，新标签页命令可完整读取。
4. 非模态文档允许祖先工具栏，但入口又要求目标 scope 等于 active scope。
   改为验证目标自身 scope，保留 allows_node 的模态限制。Firefox Reload
   修复后本地 HTTP 请求计数从 2 增到 3。
5. Activate/OpenMenu 等待有界 backend delivery 后刷新界面，不再因 effect
   观测结束取消未完成动作，也不宣称完整 effect 已验证。
6. 测试补齐标签列表、Firefox View、侧栏、全屏边界、缩放百分比断言。
   Underline 用工具栏祖先消歧；缩放先通过 GUI2TUI 设置数值初始条件。

## 三、剩余问题

- Mousepad 行号和换行：已能到达父菜单和命令，但公开 menu item 没读到
  CHECKED 变化，不能区分 provider 状态缺失与动作未生效，仍算未通过。
- FeatherPad Reload：通过 GUI2TUI 写入未保存文本的前置条件失败。目标
  暴露 EditableText、MultiLine，但有 4 个子对象；现有完整文本替换只支持
  叶节点。保留不支持，没有放宽结构保护或修改 backing file 来伪造成功。

本轮无基础设施失败或超时。未独立测量 wrong-target/unsafe，不能写成零。
Firefox 使用本地 data URL/HTTP fixture，未验收外网和 spatial 布局。
没有运行 held-out/headline benchmark，也没有冻结 benchmark。

## 四、验证与证据

Rust library tests 366 passed（含新增公开 Menu 回归）；fmt check、all-targets
check、clippy -D warnings 通过；Python contract 2 passed；语法编译、文档
审计和 diff check 通过。

- [summary.json](summary.json)：20 项结果及二进制、脚本哈希。
- [evidence.tar.gz](evidence.tar.gz)：固定运行脚本、终端记录、公开前后状态。
- [remaining-diagnostic.tar.gz](remaining-diagnostic.tar.gz)：三项残余问题的接口诊断。

## 五、后续约束

生产判断只依赖公开角色、动作、状态和 scope；应用名称与任务步骤留在测试。
有子结构文本的写回需要单独验证完整性，不能删除保护或换任务提高分数。
