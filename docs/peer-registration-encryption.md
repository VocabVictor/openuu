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
- 但 hbbs 侧目前**不支持在 TCP 上注册**：`rendezvous_server/tcp.rs` 只处理 PunchHoleRequest/RequestRelay/RelayResponse/
  PunchHoleSent/LocalAddr/TestNatRequest，收到 `RegisterPk` 回 NOT_SUPPORT，没有 `RegisterPeer` 分支；peer 表只记 UDP
  `socket_addr`，`PunchHole`/`RequestRelay`/`FetchLocalAddr` 一律往 UDP 地址发。客户端 `start_tcp` 循环也只在 key 未确认时发
  `RegisterPk`，没有周期性 `RegisterPeer` 心跳。所以方案 a 不是"切默认值"，而是先补 hbbs。

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

实施顺序（hbbs 先行，客户端默认值最后切）：
0) hbbs：TCP 上接受 `RegisterPk`/`RegisterPeer`（与 UDP 分支同样的鉴权/UUID 校验），peer 记录加 `Option<Sink>`，
   下发 `PunchHole`/`RequestRelay`/`FetchLocalAddr` 时有 sink 走 sink、否则走 UDP，心跳超时仍按 `last_reg_time`；
   补"TCP 注册与 UDP 注册行为等价"测试（心跳超时、地址更新、三种下发经加密 TCP 到达）和
   `event=peer_register transport=tcp|udp` 占比日志；旧 UDP 路径原样保留。hbbs 部署另批。
分三步：1) OpenUU 构建默认 `disable-udp=Y`（可在网络设置关掉），hbbs 日志加 `event=peer_register transport=tcp|udp`
统计占比，客户端 `start_tcp` 循环加 `REG_INTERVAL` 周期的 `RegisterPeer`（在 hbbs 部署上线后再落）；2) 打洞回归清单只用日志断言：tcp_punch、对称 NAT 回退到 relay、`RequestRelay` 经 TCP 下发，用两台被控端复测；
3) UDP 注册占比归零后再评估 c)。

## 回归清单（只用日志断言，hbbs 部署 + 客户端默认切换后，用两台被控端复测）

| 场景 | 被控端日志（server） | hbbs 日志 |
| --- | --- | --- |
| 注册走 TCP | `start tcp: <id server>` + `Connection secured`，之后每 15 s 无 `register_pk` 循环 | `event=secure_tcp from=<peer> secured=true`，`event=peer_register transport=tcp id=<id>`，60 s 后 `event=peer_transport online=N tcp=M` 中 M 计入该 peer |
| 心跳超时 | 拔网线 45 s 内被控端 `Rendezvous connection is timeout` 后重连 | `event=punch_hole … decision=offline` 在 REG_TIMEOUT 后出现，恢复后再次 `transport=tcp` |
| 直连打洞（非对称 NAT） | `PunchHole` 到达后 `punch hole sent`/直连日志 | `event=punch_hole … decision=punch nat_type=…`，投递经 sink 不经 UDP（无 `udp failure`） |
| 对称 NAT 回退 relay | `request relay` / 走 relay 的连接日志 | `event=punch_hole … decision=punch nat_type=SYMMETRIC` 后 `event=relay_request … peer=<peer addr>`，`event=relay_peer_ticket` |
| 同一内网 | `FetchLocalAddr` 处理后 `local addr` 上报 | `decision=local_addr` |
| 旧客户端（UDP） | 不变 | `transport=udp` 计数不为零，`RegisterPeerResponse` 仍经 UDP |
| 用户关闭 disable-udp | 被控端回到 `start udp:` | 该 peer 从 tcp 计数消失 |
