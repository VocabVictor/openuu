part of 'setting.dart';

Widget _pathRow(BuildContext context, String title, String path, bool exists,
    {VoidCallback? onEdit}) {
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
                ? (translate('Open folder'))
                : (translate('Folder does not exist yet')),
            onPressed: exists ? () => launchUrl(Uri.file(path)) : null,
            icon: const Icon(Icons.folder_open_outlined, size: 20)),
      ]),
      description: path);
}

String _settingDescription(BuildContext context, String label) {
  const descriptions = <String, String>{
    'Account': 'Manage your account and profile',
    'Default View Style': 'Choose how new connections scale the remote screen',
    'Default Scroll Style': 'Choose how to navigate a screen larger than the window',
    'Default Image Quality': 'Balance image quality and response time',
    'Default Codec': 'Choose the preferred video codec for new connections',
    'Default trackpad speed': 'Adjust trackpad movement speed',
    'Recording': 'Manage session recording and storage locations',
    'Scale original': 'Keep the original remote screen size',
    'Scale adaptive': 'Fit the remote screen to the current window',
    'Good image quality': 'Prioritize image clarity',
    'Balanced': 'Balance clarity and smoothness',
    'Optimize reaction time': 'Prioritize lower display latency',
    'Automatically record incoming sessions': 'Record sessions connecting to this device',
    'Automatically record outgoing sessions': 'Record sessions started from this device',
    'Theme': 'Choose the appearance of the app',
    'Language': 'Choose your display language',
    'Service': 'Manage the remote connection service on this device',
    'Enable hardware codec': 'Use hardware encoding and decoding to reduce CPU load',
    'Adaptive bitrate': 'Adjust video bitrate to network conditions',
    'Confirm before closing multiple tabs': 'Ask before closing multiple connections',
    'Open connection in new tab': 'Open new connections in a tab in this window',
    'Auto update': 'Download and install app updates automatically',
    'Check for software update on startup': 'Check for new versions when the app starts',
    'Use texture rendering': 'Display the remote screen using texture rendering',
    'Use D3D rendering': 'Render the remote screen with Direct3D',
    'Capture screen using DirectX': 'Capture this device’s screen with DirectX',
    'Enable TCP hole punching': 'Try to establish direct connections over TCP',
    'Enable UDP hole punching': 'Try to establish direct connections over UDP',
    'Enable IPv6 P2P connection': 'Allow direct device connections over IPv6',
    'Enable keyboard/mouse': 'Allow remote keyboard and mouse control',
    'Enable clipboard': 'Allow clipboard sharing between devices',
    'Enable file transfer': 'Allow file transfers through remote connections',
    'Enable audio': 'Allow audio from this device',
  };
  final english = descriptions[label];
  return english == null ? '' : translate(english);
}

Widget _settingRow(BuildContext context, String label, Widget control,
    {bool enabled = true, String? description, String? labelText}) {
  final detail = description ?? _settingDescription(context, label);
  return SettingsRow(
      label: labelText ?? translate(label),
      subtitle: detail,
      control: control,
      enabled: enabled);
}

// ignore: non_constant_identifier_names
Widget _Card(
    {required String title,
    required List<Widget> children,
    List<Widget>? title_suffix}) {
  if (isWindows && !bind.isIncomingOnly()) {
    if (children.length == 1 &&
        title_suffix == null &&
        ['Service', 'Theme', 'Language', 'Audio Input Device'].contains(title)) {
      return _group(null, [
        Builder(builder: (context) => _settingRow(context, title, children.single))
      ]);
    }
    return _group(translate(title), children,
        trailing: title_suffix == null
            ? null
            : Row(mainAxisSize: MainAxisSize.min, children: title_suffix));
  }
  return Row(
    children: [
      Flexible(
        child: SizedBox(
          width: _kCardFixedWidth,
          child: Card(
            child: Column(
              children: [
                Row(
                  children: [
                    Expanded(
                        child: Text(
                      translate(title),
                      textAlign: TextAlign.start,
                      style: TextStyle(fontSize: _kTitleFontSize),
                    )),
                    ...?title_suffix
                  ],
                ).marginOnly(left: _kContentHMargin, top: 10, bottom: 10),
                ...children
                    .map((e) => e.marginOnly(top: 4, right: _kContentHMargin)),
              ],
            ).marginOnly(bottom: 10),
          ).marginOnly(left: _kCardLeftMargin, top: 15),
        ),
      ),
    ],
  );
}

/// A settings group with the group gap above it.
Widget _group(String? title, List<Widget> children,
        {Widget? trailing, bool collapsible = false}) =>
    Padding(
        padding: const EdgeInsets.only(top: UiSpace.settingsGroupGap),
        child: SettingsGroup(
            title: title,
            titleTrailing: trailing,
            collapsible: collapsible,
            children: children));
