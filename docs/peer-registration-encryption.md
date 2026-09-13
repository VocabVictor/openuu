# 被控端注册通道加密设计稿（未实现）

## 现状

- 被控端 `RendezvousMediator` 默认走 UDP（`lifecycle.rs::start_udp`）：`RegisterPk`（id、uuid、公钥）和每个
  `REG_INTERVAL` 一次的 `RegisterPeer`（id、serial）明文发出；hbbs 用同一个 UDP 通道明文下发
  `RegisterPeerResponse`、`PunchHole`（主控的公网地址、relay 地址、nat 类型）、`RequestRelay`（relay 地址、uuid）、
  `FetchLocalAddr`。这些"对端票"决定被控端往哪打洞、连哪个 relay，被动监听可以画出拓扑，主动伪造可以把被控端引到
  假 relay。
- 客户端已经有一条加密的 TCP 通道：`disable-udp` 或 WebSocket 时走 `start_tcp`，先做 `secure_tcp`
  （hbbs 用 ID 服务器私钥签名的 KeyExchange → 客户端 sealed box 回传对称密钥 → 之后整条流用对称密钥加密），
  然后在这条长连接上收发同样的 rendezvous 消息。hbbs 侧 `rendezvous_server/secure.rs` + `io.rs` 已处理 KeyExchange。
- 主控端到 hbbs 的打洞请求本来就走 TCP + `secure_tcp`；只有被控端的注册/心跳/票据下发还在 UDP 明文。

## 候选方案

| | a) 注册改走现有加密 TCP | b) UDP 上用 sealed box 单向加密下发 | c) 全部改 TCP 长连接、去掉 UDP |
| --- | --- | --- | --- |
| 做法 | 被控端默认 `disable-udp`（或新选项 `secure-rendezvous`）走 `start_tcp` + `secure_tcp`；hbbs 保留 UDP 给旧客户端 | hbbs 用 `RegisterPk` 里的被控端公钥把 `PunchHole`/`RequestRelay`/`FetchLocalAddr` 封成 sealed box（新消息 `SealedRendezvous{nonce, payload}`）；上行 `RegisterPeer` 仍明文（只有 id/serial） | 同 a，但 hbbs 关闭 21116/UDP，客户端删掉 UDP 路径 |
| 旧客户端兼容 | 完全兼容：hbbs 两条路都开，旧客户端继续 UDP | 需要协议版本协商：旧客户端收不懂 sealed 消息，hbbs 要按 `RegisterPeer.serial`/版本字段分流 | 不兼容：旧客户端无法注册，必须先全量升级 |
| hbbs 负载 | 每个在线被控端一条长 TCP 连接（约 10 KB 内存 + 心跳），万级在线可接受；相比 UDP 多一次握手和 keepalive | 几乎不变：只多每条下行一次 crypto_box（微秒级），无连接状态 | 同 a |
| 改动范围 | 客户端：默认值/选项一行 + 文档；hbbs：确认 TCP 连接上的 `RegisterPeer`/心跳、`PunchHole` 推送与 UDP 路径行为一致（`io.rs`/`register` 处理，现有代码已有骨架）；无协议改动 | 客户端：新消息解密、重放/nonce 检查、RegisterPk 未完成前的回退；hbbs：密钥缓存、封装、版本分流；protobuf 加消息（hbb_common 子模块要过一轮） | a 的改动 + 删 UDP 代码 + 运维（防火墙/文档） |
| 剩余风险 | UDP 明文路径仍在（旧客户端），需要日志/统计推动升级；NAT 类型探测（21115）不受影响 | 上行仍明文；只加密不认证发送方时要靠签名（hbbs 私钥签 payload）；实现量最大 | 一次性切换风险 |

## 推荐

推荐 a)：复用已经落地并在两端都有测试的 `secure_tcp` KeyExchange，把被控端注册切到加密 TCP，UDP 只为旧客户端保留。
理由：零协议改动、hbb_common 子模块不用动、旧客户端不受影响、hbbs 已经在为主控端维持同样的 TCP 会话；
b) 的收益（无连接状态）对我们规模不重要且实现最重；c) 是 a) 全量升级完成后的收尾，而不是起点。

分三步：1) OpenUU 构建默认 `disable-udp=Y`（可在网络设置关掉），hbbs 日志加 `event=peer_register transport=tcp|udp`
统计占比；2) hbbs 对 TCP 注册补齐与 UDP 等价的行为测试（心跳超时、`PunchHole` 推送、断线重连）；
3) UDP 注册占比归零后再评估 c)。
