# 本地应用查看资源：同机与 SSH

GUI2TUI 操作远端 GUI 的公开语义，本地应用查看明确提供的资源文件。
支持命令行及 F4 显式文件发送；自动从 GUI 获取原文件尚未完成。
不要求资源来自特定应用，当前格式受 broker 的 MIME 支持范围约束。

## 同机

在本地终端启动接收端。macOS 使用 /usr/bin/open；Linux 可使用本机已安装的
xdg-open 的绝对路径。处理器启动成功只表示接收，不能证明画面已经正确呈现。

    client_dir=$(mktemp -d)
    chmod 700 "$client_dir"
    gui2tui endpoint serve --socket "$client_dir/broker.sock" +      --mime 'image/*' --mime application/pdf --mime 'audio/*' --mime 'video/*' +      --handler-program /usr/bin/open

在另一个终端，用上面实际生成的路径替换 /PRIVATE/broker.sock：

    gui2tui endpoint send-artifact --socket /PRIVATE/broker.sock +      --input /absolute/path/document.pdf --mime application/pdf --kind document

接收端询问 Once / Session / Deny。显式自动化可配置 --authorization once。
图片使用相应 MIME 和 --kind image，音视频使用 audio/video；未知格式不能仅通过
修改 MIME 伪装为已支持资源。显式文件路径不是从窗口标题猜出的源文件。

## SSH：远端发送到当前电脑

在 GUI2TUI 所在主机的配置文件保存 [resources]，设置 socket 为转发 socket 的
绝对路径、remote = true；或使用 --modality-socket PATH --modality-remote。
同机配置 remote = false。远端模式明确禁用 Enter 引用打开和 o 同机打开，仅保留
f 文件字节发送。配置不自动建立 SSH 连接，隧道与本地 broker 仍按下文启动。

F4 中按 f 输入当前 GUI2TUI 所在主机上的绝对文件路径，Enter 发送，Esc 取消输入。
传输期间 x 请求取消。支持 PNG/JPEG/SVG/GIF/WebP、PDF、MP4/WebM、MP3/OGG/WAV
及自包含 GLB；扩展名用于格式提示，不代表内容校验或查看器隔离。需要配置
--modality-socket，接收端仍按自己的 MIME 和授权规则决定是否接收。
F4 的显式文件与所选 GUI 对象无自动关联，也可能不含 GUI 未保存的修改。

接收端仍运行在当前电脑。在远端创建当前用户独占的目录：

    ssh USER@HOST 'mkdir -p ~/.cache/gui2tui-handoff && chmod 700 ~/.cache/gui2tui-handoff'

用实际绝对路径建立反向 Unix socket 转发；保持该 SSH 进程存活：

    ssh -N -o ExitOnForwardFailure=yes +      -R /REMOTE/PRIVATE/broker.sock:/LOCAL/PRIVATE/broker.sock USER@HOST

远端执行上面的 send-artifact，socket 指向 /REMOTE/PRIVATE/broker.sock，input
指向远端文件。SSH 负责认证和加密，broker 校验长度及 SHA-256 后调用本地处理器。
两端路径可以不同。保留正常 SSH 主机身份校验，不需要公开 TCP broker 端口。
远端目录和本地 socket 都应归对应用户独占；已有 socket 不自动删除或覆盖。
停止后仅清理本次建立的远端 socket。转发失败应先检查 SSH 服务的 stream-local
转发策略，不通过降低授权或扩大目录权限绕过。

跨主机必须发送 artifact 字节；远端文件路径和 localhost URL 并不会自动变为
本地可访问资源。当前连接不能自动识别远端身份或转译引用，因此暂不要把 SSH
转发 socket 用于 TUI 的 Enter 引用或 o 同机查看入口。配置给 TUI 后应使用
f 的字节传输入口；远端路径不会被作为接收端的本地路径打开。

## 验证范围

2026-10-09：macOS 接收端同机 PNG/PDF/WAV 传输及真实系统打开器接受通过；
Linux amd64 隔离容器经 OpenSSH 反向 Unix socket 转发至 macOS，PDF 交付通过，
传输前取消验证 payload_sent=0。这验证了跨操作系统进程和 SSH 链路，未模拟广域网。
源/接收程序使用现有协议，测试没有将个人文件传出。

详细结果见[验证记录](validation/local-resource-20261009/README.md)。SSH 中途断线、
大文件、拒绝、清理及多类型覆盖仍需补充；不能用既有单元测试冒充这些端到端实测。
