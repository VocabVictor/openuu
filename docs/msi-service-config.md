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

- `--import-config` 只在服务侧文件比用户文件旧（或不存在）时写入，升级安装不会覆盖已有的服务侧配置。
- 已弃用的方案：让 `--server` 首次启动时读活动用户的 `%APPDATA%`（`bbe6fcc73`，已 revert）；
  多用户机器上"活动用户"不确定，且服务进程读用户文件属于新增行为。
