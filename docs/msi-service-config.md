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
- 自 `bbe6fcc73` 起，服务首次启动 `--server` 时若服务侧还没有配置文件，会自动导入当前活动用户的配置；
  已经装好的机器不会再触发，需按下面步骤手工配置一次。

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

## 待办

MSI 自定义动作 `CreateStartService`（`res/msi/CustomActions/CustomActions.cpp`）仍未做 `--import-config`，
装机时的 ID 会与便携模式不同；如需保持 ID 一致，需在 MSI 里补上与 `install_me` 相同的临时服务导入步骤。
