# 异网冒烟方案（跨公网出口）

至今所有连通性验证都在同一条家庭宽带内完成：主控与被控端共用一个出口 NAT，
hbbs 走 `decision=local_addr` 或同网段打洞，中继只在人为强制时出现。下面三项
因此一直挂着，它们都只有在两端出口不同时才会自然发生。本方案给出环境候选、
命令与日志断言，**批准后再执行**；不自行购买或开通任何机器。

## 1. 待验证的三项

| 编号 | 项目 | 自然触发条件 |
| --- | --- | --- |
| T1 | 跨 NAT 直连打洞 | 两端出口不同、至少一端是锥形 NAT |
| T2 | 对称 NAT 回退中继 | 一端出口为对称 NAT（或客户端 `force-always-relay`） |
| T3 | 被控端自发起中继的对端票 | 被控端在打洞失败后自己发起 `RelayResponse` |

## 2. 环境候选与覆盖

| 候选 | 拓扑 | 覆盖 | 限制 |
| --- | --- | --- | --- |
| A. 手机热点 + 笔记本做主控 | 主控在运营商 CGNAT 后，被控端仍在家庭宽带后的虚机 | T1（热点为锥形时）、T2（热点为对称时）、T3 | NAT 类型由运营商决定、不可选；每次实验都要断开笔记本的有线/无线网，会中断本机其它会话；流量走手机套餐 |
| B. 云主机跑被控端 | 被控端有独立公网 IP（1:1 NAT），主控在家庭宽带后 | T1 最稳（几乎必成）、T2/T3 需人为强制 | 需要一台已有的云主机；云安全组要放行 UDP/TCP 的打洞端口；等同"公网直连"，不能代表真实对称 NAT 场景 |
| C. 公司网络 | 主控在公司出口，被控端在家庭宽带后 | 取决于公司出口，通常是对称 NAT + UDP 受限，偏向 T2/T3 | 出口策略不可控、可能整段封 UDP；需要在办公环境操作 |

**建议组合**：先做 A（一次实验覆盖三项中的两到三项，零成本），再用 B 复核 T1
的成功路径。C 只在 A、B 都无法得出结论时考虑。

## 3. 前置准备（两端各一次）

* 两端都装当前 master 的构建；被控端以服务方式运行，主控用便携版或安装版。
* 服务端开启 `RUST_LOG=debug` 已是常态；本方案只读日志，不改服务端配置。
  **不要**为 T2 设置 hbbs 的 `ALWAYS_USE_RELAY`：那是全局开关，会影响别人正在
  跑的测量；强制中继一律在客户端侧按需开启。
* 记录被控端 ID（`openuu.exe --get-id`）与两端的公网出口（`curl -s ifconfig.me`），
  确认两个出口不同再开始。

## 4. 步骤、命令与断言

每轮实验的主控侧命令都是同一条（密码从私有文件读取，不手打进命令行）：

```
openuu.exe --connect <peer-id> --password <从 test-peers.txt 读取>
```

判定材料有三处：主控客户端日志、被控端服务日志、服务端 hbbs/hbbr 日志。

### T1 跨 NAT 直连打洞

1. 两端出口不同，主控发起连接。
2. 断言：
   * 主控日志出现 `TCP Hole Punched <peer-id> = <addr>`（或 `UDP Hole Punched`），
     其中 `<addr>` 是被控端的**公网**地址，不是内网地址；
   * hbbs 日志出现 `event=punch_hole from=… id=… peer=… decision=punch nat_type=…`，
     且 `nat_type` 不是 `SYMMETRIC`；
   * hbbr 日志在该时间窗内**没有**这次 uuid 的 `Relayrequest … got paired`；
   * 主控日志 `… used to establish Direct connection`（`typ` 为 `Direct`）。
3. 失败时记录 `decision=` 的取值与 `nat_type`，它直接说明是判定走了中继还是打洞超时。

### T2 对称 NAT 回退中继

1. 若候选环境自然给出对称 NAT，直接连；否则在主控侧该 peer 的
   `config/peers/<peer-id>.toml` 的 `[options]` 里加 `force-always-relay = 'Y'`
   （等价于界面上的"始终通过中继连接"），再连。
2. 断言：
   * 主控日志 `relay requested from peer, time used: …, relay_server: <host>`；
   * hbbs 日志 `event=relay_request from=… id=… peer=… relay=… uuid=<uuid>`；
   * hbbr 日志先 `New relay request <uuid> from <addr>`、后
     `Relayrequest <uuid> from <addr> got paired`（两端都到齐才算中继真正建立）；
   * 画面可用，且主控日志没有 `Hole Punched`。
3. 用完把 `force-always-relay` 删掉，避免污染后续轮次。

### T3 被控端自发起中继的对端票

1. 触发路径是被控端在打洞失败后自己发起中继，所以要让**被控端**这一侧无法打洞：
   优先用候选 C/A 中被控端处于对称 NAT 的组合；退而求其次，在被控端的
   `config/peers/<controller-id>.toml` 里同样置 `force-always-relay = 'Y'`。
2. 断言（这一项的关键证据在服务端）：
   * hbbs 日志 `event=relay_response from=… to=… relay=… refuse=false`；
   * 紧随其后 `event=relay_peer_ticket uuid=<uuid> via=response` —— 这条就是
     "被控端自发起的中继也拿到了对端票"，是本项唯一的正面证据；
   * hbbr 日志 `Relayrequest <uuid> … got paired`，且**没有**
     `event=relay_denied from=… relay=<uuid>`；
   * 被控端服务日志没有回落到自行登录取票的路径。
3. 反例同样有价值：若出现 `event=relay_denied` 或只有 `New relay request` 而无
   `got paired`，说明票没发到或已过期（票 60 s），记录时间差。

### 联查方法

三处日志用 uuid 串起来：

```
# hbbs
grep -E 'event=punch_hole|event=relay_request|event=relay_response|event=relay_peer_ticket'
# hbbr
grep -E 'New relay request|got paired|event=relay_denied'
```

## 5. 一键脚本

三项的命令与断言已经写成 `scripts/wan-smoke.ps1`：

```
pwsh -File scripts/wan-smoke.ps1 -Peer <peer-id> -Case all -ServerSsh <ssh 别名> -Out .\wan-smoke
```

它每轮起一次 `--connect`，按上面的断言判定并输出 PASS/FAIL 表格与 csv，有失败时退出码为 1。
约束与本方案一致：服务端只读（ssh 跑 `journalctl` 抓那几条 `event=` 行），全局配置一律不动；
唯一的写入是主控侧该 peer 的 `force-always-relay`，改前做带时间戳的备份，恢复后比对文件哈希，
哈希一致才删备份，且写在 `finally` 里，中途失败也会恢复。密码按 peer id 从私有文件提取，
不进命令行历史也不入日志：客户端 stdout 会回显启动参数，脚本只保留判定用的行并当场删除原始输出。

T3 需要**被控端**打洞失败，脚本不会去改被控端的配置，那一步仍由人来做（或靠环境天然的对称 NAT）。

## 6. 耗时与风险

* 准备（确认出口不同、装包、记录 ID）：约 30 分钟。
* 每项实验含日志采集与断言：15–25 分钟；三项一轮约 1 小时。
* 若候选 A 的 NAT 类型不配合（既非对称也打不通），需要换候选重来，追加约 1 小时。
* 风险：候选 A 会临时切断笔记本的现有网络，做之前要先跟正在用编译机/虚机的会话
  打招呼；云主机候选要确认安全组，否则打洞失败会被误判成代码问题。
* 全程命令行，不需要图形界面操作，符合"验证只走命令行"的规定。
