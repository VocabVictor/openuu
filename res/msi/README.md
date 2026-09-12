# OpenUU msi project

WiX v4 project (SDK-style `Package.wixproj` + native `CustomActions.vcxproj`).
Derived from the RustDesk msi project, which itself came from
<https://github.com/MediaPortal/MediaPortal-2.git>.

Naming is driven by `preprocess.py --app-name` (default `OpenUU`): product name,
install folder (`Program Files\OpenUU`), start-menu folder, shortcuts, the Windows
service (`OpenUU`, running `OpenUU.exe --service`, auto start) and the registry
keys all derive from it. The `UpgradeCode` is `uuid5(NAMESPACE_OID, "<app>.exe")`,
i.e. `4ec5e3f7-0cc7-5df5-b528-ec83260dfaa0` for OpenUU, so an OpenUU package never
upgrades or collides with a RustDesk installation.

## Steps (CI / a machine with VS2022 + .NET SDK)

1. `python preprocess.py -d <dist dir>` (see `python preprocess.py -h`). The dist
   dir is the Flutter bundle (`openuu.exe`, `libopenuu.dll`, `data/`, ...).
   The script rewrites the `.wxs` files in place; run it on a copy or `git checkout -- res/msi` afterwards.
2. `nuget restore msi.sln`
3. `msbuild msi.sln -p:Configuration=Release -p:Platform=x64 /p:TargetVersion=Windows10`
4. Output: `Package/bin/x64/Release/en-us/Package.msi`

`.github/workflows/flutter-build.yml` does exactly this on `windows-2022`.

## Steps (VS2019 BuildTools only, no system-wide installs)

MSBuild 16.11 cannot host the SDK-style wixproj, so build the two projects separately:

1. Portable .NET SDK: `dotnet-install.ps1 -Channel 8.0 -InstallDir <dir> -NoPath`
   (WiX 4.0.5 targets net6.0 and rolls forward to 8.0).
2. `nuget.exe restore CustomActions\packages.config -PackagesDirectory packages`
3. `MSBuild.exe CustomActions\CustomActions.vcxproj -p:Configuration=Release -p:Platform=x64 -p:PlatformToolset=v142`
4. `python preprocess.py -d <dist dir>`
5. `dotnet build Package\Package.wixproj -c Release -p:Platform=x64 -p:CustomActionsDll=<abs path to CustomActions.dll>`
   `CustomActionsDll` makes the wixproj skip its `ProjectReference` and defines
   `CustomActions.TargetDir` / `CustomActions.TargetName` from the given path.

`openuu-toolchain/build-msi.ps1` automates this on the development machine. Make sure
a `NuGet.Config` with the nuget.org source is in effect (the machine-wide one may have none).

Run `msiexec /i package.msi /l*v install.log` to record the log.

## Usage

1. Put the custom dialog bitmaps in "Resources" directory. The supported bitmaps are `['WixUIBannerBmp', 'WixUIDialogBmp', 'WixUIExclamationIco', 'WixUIInfoIco', 'WixUINewIco', 'WixUIUpIco']`.

## Knowledge

### properties

[wix-toolset-set-custom-action-run-only-on-uninstall](https://www.advancedinstaller.com/versus/wix-toolset/wix-toolset-set-custom-action-run-only-on-uninstall.html)

| Property Name | Install | Uninstall | Change | Repair | Upgrade |
| ------ | ------ | ------ | ------ | ------ | ------ |
| Installed | False | True | True | True | True |
| REINSTALL | False | False | False | True | False |
| UPGRADINGPRODUCTCODE | False | False | False | False | True |
| REMOVE | False | True | False | False | True |

## Refs

1. [windows-installer-portal](https://learn.microsoft.com/en-us/windows/win32/Msi/windows-installer-portal)
1. [wxs](https://wixtoolset.org/docs/schema/wxs/)
1. [wxs github](https://github.com/wixtoolset/wix)
