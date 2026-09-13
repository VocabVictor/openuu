part of 'desktop_setting_page.dart';

Widget _pathRow(BuildContext context, String title, String path, bool exists,
    {VoidCallback? onEdit}) {
  final zh = Localizations.localeOf(context).languageCode == 'zh';
  return _settingRow(
      context,
      title,
      Row(mainAxisSize: MainAxisSize.min, children: [
        if (onEdit != null)
          IconButton(
              tooltip: translate('Change'),
              onPressed: onEdit,
              icon: const Icon(Icons.edit_outlined, size: 20)),
        IconButton(
            tooltip: exists
                ? (zh ? '打开文件夹' : 'Open folder')
                : (zh ? '文件夹尚未创建' : 'Folder does not exist yet'),
            onPressed: exists ? () => launchUrl(Uri.file(path)) : null,
            icon: const Icon(Icons.folder_open_outlined, size: 20)),
      ]),
      description: path);
}

String _settingDescription(BuildContext context, String label) {
  final zh = Localizations.localeOf(context).languageCode == 'zh';
  const descriptions = <String, List<String>>{
    'Account': ['管理登录账号与个人信息', 'Manage your account and profile'],
    'Default View Style': [
      '设置新连接的画面缩放方式',
      'Choose how new connections scale the remote screen'
    ],
    'Default Scroll Style': [
      '设置远程画面超出窗口时的滚动方式',
      'Choose how to navigate a screen larger than the window'
    ],
    'Default Image Quality': [
      '平衡画面清晰度与响应速度',
      'Balance image quality and response time'
    ],
    'Default Codec': [
      '设置新连接优先使用的视频编码',
      'Choose the preferred video codec for new connections'
    ],
    'Default trackpad speed': ['调整触控板移动速度', 'Adjust trackpad movement speed'],
    'Recording': [
      '管理会话自动录制与文件存储位置',
      'Manage session recording and storage locations'
    ],
    'Scale original': ['保持远程画面的原始尺寸', 'Keep the original remote screen size'],
    'Scale adaptive': [
      '根据当前窗口大小缩放画面',
      'Fit the remote screen to the current window'
    ],
    'Good image quality': ['优先保证画面清晰度', 'Prioritize image clarity'],
    'Balanced': ['兼顾清晰度与流畅度', 'Balance clarity and smoothness'],
    'Optimize reaction time': [
      '优先减少画面传输延迟',
      'Prioritize lower display latency'
    ],
    'Automatically record incoming sessions': [
      '自动保存其他设备连接本机的会话录像',
      'Record sessions connecting to this device'
    ],
    'Automatically record outgoing sessions': [
      '自动保存本机发起的会话录像',
      'Record sessions started from this device'
    ],
    'Theme': ['选择应用的显示外观', 'Choose the appearance of the app'],
    'Language': ['选择界面使用的语言', 'Choose your display language'],
    'Service': [
      '管理此设备的远程连接服务',
      'Manage the remote connection service on this device'
    ],
    'Enable hardware codec': [
      '使用硬件编解码，减轻处理器负担',
      'Use hardware encoding and decoding to reduce CPU load'
    ],
    'Adaptive bitrate': [
      '根据网络状况自动调整画面码率',
      'Adjust video bitrate to network conditions'
    ],
    'Confirm before closing multiple tabs': [
      '关闭多个连接前再次确认，避免误操作',
      'Ask before closing multiple connections'
    ],
    'Open connection in new tab': [
      '在当前窗口的标签页中打开新的连接',
      'Open new connections in a tab in this window'
    ],
    'Auto update': [
      '自动获取并安装应用更新',
      'Download and install app updates automatically'
    ],
    'Check for software update on startup': [
      '启动应用时检查可用的新版本',
      'Check for new versions when the app starts'
    ],
    'Use texture rendering': [
      '使用纹理渲染显示远程画面',
      'Display the remote screen using texture rendering'
    ],
    'Use D3D rendering': [
      '使用 Direct3D 渲染远程画面',
      'Render the remote screen with Direct3D'
    ],
    'Capture screen using DirectX': [
      '使用 DirectX 捕获此设备的屏幕',
      'Capture this device’s screen with DirectX'
    ],
    'Enable TCP hole punching': [
      '尝试通过 TCP 建立设备间的直接连接',
      'Try to establish direct connections over TCP'
    ],
    'Enable UDP hole punching': [
      '尝试通过 UDP 建立设备间的直接连接',
      'Try to establish direct connections over UDP'
    ],
    'Enable IPv6 P2P connection': [
      '允许设备通过 IPv6 直接连接',
      'Allow direct device connections over IPv6'
    ],
    'Enable keyboard/mouse': [
      '允许远程控制此设备的键盘和鼠标',
      'Allow remote keyboard and mouse control'
    ],
    'Enable clipboard': [
      '允许在两台设备之间共享剪贴板',
      'Allow clipboard sharing between devices'
    ],
    'Enable file transfer': [
      '允许远程连接传输文件',
      'Allow file transfers through remote connections'
    ],
    'Enable audio': ['允许传输此设备的声音', 'Allow audio from this device'],
  };
  return descriptions[label]?[zh ? 0 : 1] ?? '';
}

Widget _settingRow(BuildContext context, String label, Widget control,
    {bool enabled = true, String? description}) {
  final detail = description ?? _settingDescription(context, label);
  return SettingsRow(
      label: translate(label),
      subtitle: detail,
      control: control,
      enabled: enabled);
}

// ignore: non_constant_identifier_names
Widget _Card(
    {required String title,
    required List<Widget> children,
    List<Widget>? title_suffix}) {
  if (isWindows &&
      !bind.isIncomingOnly() &&
      children.length == 1 &&
      title_suffix == null &&
      ['Service', 'Theme', 'Language', 'Audio Input Device'].contains(title)) {
    return Card(
        margin: const EdgeInsets.only(left: _kCardLeftMargin, top: 8),
        child: Builder(
            builder: (context) => LayoutBuilder(
                builder: (context, bounds) => _settingRow(
                    context,
                    title,
                    SizedBox(
                        width: bounds.maxWidth < 600 ? 160 : 220,
                        child: children.single)))));
  }
  return Row(
    children: [
      Flexible(
        child: SizedBox(
          width: isWindows && !bind.isIncomingOnly()
              ? double.infinity
              : _kCardFixedWidth,
          child: Card(
            child: Column(
              children: [
                if (isWindows && !bind.isIncomingOnly())
                  Builder(
                      builder: (context) => _settingRow(
                          context,
                          title,
                          Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [...?title_suffix])))
                else
                  Row(
                    children: [
                      Expanded(
                          child: Text(
                        translate(title),
                        textAlign: TextAlign.start,
                        style: TextStyle(
                          fontSize: isWindows && !bind.isIncomingOnly()
                              ? 16
                              : _kTitleFontSize,
                        ),
                      )),
                      ...?title_suffix
                    ],
                  ).marginOnly(left: _kContentHMargin, top: 10, bottom: 10),
                ...children
                    .map((e) => e.marginOnly(top: 4, right: _kContentHMargin)),
              ],
            ).marginOnly(bottom: 10),
          ).marginOnly(
              left: _kCardLeftMargin,
              top: isWindows && !bind.isIncomingOnly() ? 8 : 15),
        ),
      ),
    ],
  );
}
