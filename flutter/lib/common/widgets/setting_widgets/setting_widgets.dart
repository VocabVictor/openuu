import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
part 'image_quality.dart';
part 'trackpad_speed.dart';

List<Widget> ServerConfigImportExportWidgets(
  List<TextEditingController> controllers,
  List<RxString> errMsgs,
) {
  import() {
    Clipboard.getData(Clipboard.kTextPlain).then((value) {
      importConfig(controllers, errMsgs, value?.text);
    });
  }

  export() {
    final text = ServerConfig(
            idServer: controllers[0].text.trim(),
            relayServer: controllers[1].text.trim(),
            apiServer: controllers[2].text.trim(),
            key: controllers[3].text.trim())
        .encode();
    debugPrint("ServerConfig export: $text");
    Clipboard.setData(ClipboardData(text: text));
    showToast(translate('Export server configuration successfully'));
  }

  return [
    Tooltip(
      message: translate('Import server config'),
      child: IconButton(
          icon: Icon(Icons.paste, color: Colors.grey), onPressed: import),
    ),
    Tooltip(
        message: translate('Export Server Config'),
        child: IconButton(
            icon: Icon(Icons.copy, color: Colors.grey), onPressed: export))
  ];
}

List<(String, String)> otherDefaultSettings() {
  List<(String, String)> v = [
    ('View Mode', kOptionViewOnly),
    if ((isDesktop))
      ('show_monitors_tip', kKeyShowMonitorsToolbar),
    if ((isDesktop))
      ('Collapse toolbar', kOptionCollapseToolbar),
    ('Show remote cursor', kOptionShowRemoteCursor),
    ('Follow remote cursor', kOptionFollowRemoteCursor),
    ('Follow remote window focus', kOptionFollowRemoteWindow),
    if ((isDesktop)) ('Zoom cursor', kOptionZoomCursor),
    ('Show quality monitor', kOptionShowQualityMonitor),
    ('Mute', kOptionDisableAudio),
    if (isDesktop) ('Enable file copy and paste', kOptionEnableFileCopyPaste),
    ('Disable clipboard', kOptionDisableClipboard),
    ('Lock after session end', kOptionLockAfterSessionEnd),
    ('Privacy mode', kOptionPrivacyMode),
    ('True color (4:4:4)', kOptionI444),
    ('Reverse mouse wheel', kKeyReverseMouseWheel),
    ('swap-left-right-mouse', kOptionSwapLeftRightMouse),
    if (isDesktop)
      (
        'Show displays as individual windows',
        kKeyShowDisplaysAsIndividualWindows
      ),
    if (isDesktop)
      (
        'Use all my displays for the remote session',
        kKeyUseAllMyDisplaysForTheRemoteSession
      ),
    ('Keep terminal sessions on disconnect', kOptionTerminalPersistent),
    (
      'Allow terminal apps to copy to clipboard',
      kOptionAllowTerminalClipboardWrite
    ),
  ];

  return v;
}

String getOtherDefaultSettingOption(String key) {
  if (key == kOptionAllowTerminalClipboardWrite) {
    return bind.mainGetLocalOption(key: key);
  }
  return bind.mainGetUserDefaultOption(key: key);
}

Future<void> setOtherDefaultSettingOption(String key, String value) {
  if (key == kOptionAllowTerminalClipboardWrite) {
    return bind.mainSetLocalOption(
      key: key,
      value: value == kTerminalClipboardWriteAllowed
          ? kTerminalClipboardWriteAllowed
          : kTerminalClipboardWriteDenied,
    );
  }
  return bind.mainSetUserDefaultOption(key: key, value: value);
}

bool isOtherDefaultSettingReadOnly(String key) =>
    isOptionFixed(key) ||
    (key == kOptionAllowTerminalClipboardWrite && bind.isDisableSettings());
