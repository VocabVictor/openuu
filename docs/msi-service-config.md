# MSI 安装后服务侧配置

## 现象

MSI 安装（服务模式）后登录框报
`RequestException, statusCode: 0, error: Configure an HTTP or HTTPS OpenUU account server in Network settings`，
`%APPDATA%\OpenUU\config\OpenUU2.toml` 里的 `api-server` 与 `key` 被抹掉，手工补回后再次消失。

## 根因

- 服务模式下真正生效的配置在 SYSTEM 账户的
  `C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\OpenUU\config\`（普通用户无读权限）。
- 应用内安装（`install_me`）会先用临时服务跑 `OpenUU.exe --import-config <用户配置>`，
  把用户的 ID/密码/选项复制到服务侧；MSI 的 `CreateStartService` 直接 `sc create … --service`，没有这一步，
  服务侧配置从零开始（新 ID、无 `api-server`/`key`）。
- GUI 启动时通过 IPC `SyncConfig` 用服务侧配置覆盖用户文件，所以用户文件里的两项会被"抹掉"。
- 修复：MSI 的 `CreateStartService` 现在先用临时服务 `<app>ConfigImport` 跑
  `OpenUU.exe --import-config "[AppDataFolder]OpenUU\config\OpenUU.toml"`（安装发起用户的配置），
  再创建正式服务，与 `install_me` 一致。已用旧 MSI 装好的机器需按下面步骤手工配置一次。

## 已装机器的配置方法（管理员命令行）

必须用安装目录里的 exe（主 IPC 会校验对端 exe 路径，其他目录的副本读回为空）：

```powershell
cd "D:\Program Files\OpenUU"
.\OpenUU.exe --option custom-rendezvous-server rs.example.com:21116
.\OpenUU.exe --option relay-server rs.example.com:21117
.\OpenUU.exe --option api-server http://rs.example.com:21114
.\OpenUU.exe --option key <ID 服务器公钥>
.\OpenUU.exe --option api-server   # 回读确认
```

GUI 每秒从服务同步一次选项，无需重启即可登录。服务侧文件可用管理员 PowerShell 查看：
`Get-Content C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\OpenUU\config\OpenUU2.toml`。

## 说明

- `--import-config`（`src/core_main/import_config.rs`）对两个文件各自决定：`OpenUU.toml`（ID/密码）只在非空、比服务侧新且早于 exe 时写入；
  `OpenUU2.toml`（选项）在服务侧文件不存在、服务侧没有 `custom-rendezvous-server`、或用户侧更新时导入，
  已存在的服务侧文件保留自己的其它键（如 `pinned-windows-session`），只叠加用户侧的选项。
  旧逻辑在用户侧 `OpenUU.toml` 为空时整体跳过、且 `OpenUU2.toml` 只看修改时间，会让服务侧缺少服务器四项（`a1443182b` 修复，带单元测试）。
- 已弃用的方案：让 `--server` 首次启动时读活动用户的 `%APPDATA%`（`bbe6fcc73`，已 revert）；
  多用户机器上"活动用户"不确定，且服务进程读用户文件属于新增行为。

## 装机验证（2026-09-13，Hyper-V 虚机 vm）

MSI `OpenUU-1.5.0-x86_64.msi`（编译机 build-flutter 8f = master c120a979e，含 a1443182b；sha256 前缀 bb729afe476231f5），
每轮前停删旧服务并删除 `C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\OpenUU`，用户侧只保留含四项的 `OpenUU2.toml`。

| 轮次 | 用户侧 `OpenUU.toml` | 服务侧 `OpenUU2.toml` 四项 | import-config 日志 | server 日志 |
| --- | --- | --- | --- | --- |
| 1 | 空文件（0 字节） | custom-rendezvous-server / relay-server / api-server / key 全部落位 | `Empty source config, skipped`（只跳过它自己） | `start rendezvous mediator of rs.example.com:21116` |
| 2 | `--get-id` 生成的 287 字节（只有 id，无 key_pair） | 同上 | 同上：`Config::is_empty` 要求有 key_pair，所以只含 id 的文件仍按空处理，ID 由服务侧重新生成（本机 gen_id 结果相同） | 同上 |

两轮 MSI 日志都能看到 `CreateStartService` 收到 `…--service|C:\Users\user\AppData\Roaming\OpenUU\config\OpenUU.toml` 并执行 `Import user config`。
验证脚本：会话 scratchpad 的 `vm-msi-verify.ps1`（backup / 1 / 2 / restore 四步）。
