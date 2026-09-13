import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';

import '../consts.dart';
import '../models/model.dart';
import '../models/platform_model.dart';

import 'peer_widgets.dart';

/// find ffi, tag is Remote ID
/// for session specific usage
FFI ffi(String? tag) {
  return Get.find<FFI>(tag: tag);
}

/// Global FFI object
late FFI _globalFFI;

FFI get gFFI => _globalFFI;

Future<void> initGlobalFFI() async {
  debugPrint("_globalFFI init");
  _globalFFI = FFI(null);
  debugPrint("_globalFFI init end");
  // after `put`, can also be globally found by Get.find<FFI>();
  Get.put<FFI>(_globalFFI, permanent: true);
}

String translate(String name) {
  if (name.startsWith('Failed to') && name.contains(': ')) {
    return name.split(': ').map((x) => translate(x)).join(': ');
  }
  try {
    return platformFFI.translate(name, localeName).replaceAll('RustDesk', 'OpenUU');
  } on Error {
    // The table lives behind the bridge, which a widget test does not have.
    // A key is its own English text (AGENTS.md), so the key is the answer
    // rather than a placeholder, and a widget under test renders as it does
    // in English instead of throwing.
    return name;
  }
}

// This function must be kept the same as the one in rust and sciter code.
// rust: libs/hbb_common/src/config.rs -> option2bool()
// sciter: Does not have the function, but it should be kept the same.
bool option2bool(String option, String value) {
  bool res;
  if (option.startsWith("enable-")) {
    res = value != "N";
  } else if (option.startsWith("allow-") ||
      option == kOptionStopService ||
      option == kOptionDirectServer ||
      option == kOptionForceAlwaysRelay) {
    res = value == "Y";
  } else {
    // "" is true
    res = value != "N";
  }
  return res;
}

String bool2option(String option, bool b) {
  String res;
  if (option.startsWith('enable-') &&
      option != kOptionEnableUdpPunch &&
      option != kOptionEnableIpv6Punch &&
      option != kOptionEnableWebrtc) {
    res = b ? defaultOptionYes : 'N';
  } else if (option.startsWith('allow-') ||
      option == kOptionStopService ||
      option == kOptionDirectServer ||
      option == kOptionForceAlwaysRelay) {
    res = b ? 'Y' : defaultOptionNo;
  } else {
    res = b ? 'Y' : 'N';
  }
  return res;
}

mainSetBoolOption(String key, bool value) async {
  String v = bool2option(key, value);
  await bind.mainSetOption(key: key, value: v);
}

Future<bool> mainGetBoolOption(String key) async {
  return option2bool(key, await bind.mainGetOption(key: key));
}

bool mainGetBoolOptionSync(String key) {
  return option2bool(key, bind.mainGetOptionSync(key: key));
}

mainSetLocalBoolOption(String key, bool value) async {
  String v = bool2option(key, value);
  await bind.mainSetLocalOption(key: key, value: v);
}

bool mainGetLocalBoolOptionSync(String key) {
  return option2bool(key, bind.mainGetLocalOption(key: key));
}

bool mainGetPeerBoolOptionSync(String id, String key) {
  return option2bool(key, bind.mainGetPeerOptionSync(id: id, key: key));
}

Future<bool> matchPeer(
    String searchText, Peer peer, PeerTabIndex peerTabIndex) async {
  if (searchText.isEmpty) {
    return true;
  }
  if (peer.id.toLowerCase().contains(searchText)) {
    return true;
  }
  if (peer.hostname.toLowerCase().contains(searchText) ||
      peer.username.toLowerCase().contains(searchText)) {
    return true;
  }
  if (peer.alias.toLowerCase().contains(searchText)) {
    return true;
  }
  if (peerTabShowNote(peerTabIndex) &&
      peer.note.toLowerCase().contains(searchText)) {
    return true;
  }
  return false;
}

/// Get the image for the current [platform].
Widget getPlatformImage(String platform, {double size = 50}) {
  if (platform.isEmpty) {
    return Container(width: size, height: size);
  }
  if (platform == kPeerPlatformMacOS) {
    platform = 'mac';
  } else if (platform != kPeerPlatformLinux &&
      platform != kPeerPlatformAndroid) {
    platform = 'win';
  } else {
    platform = platform.toLowerCase();
  }
  return SvgPicture.asset('assets/$platform.svg', height: size, width: size);
}

isOptionFixed(String key) => bind.mainIsOptionFixed(key: key);

bool isChangePermanentPasswordDisabled() =>
    bind.mainGetBuildinOption(key: kOptionDisableChangePermanentPassword) ==
    'Y';

bool isChangeIdDisabled() =>
    bind.mainGetBuildinOption(key: kOptionDisableChangeId) == 'Y';

bool isUnlockPinDisabled() =>
    bind.mainGetBuildinOption(key: kOptionDisableUnlockPin) == 'Y';

bool? _isCustomClient;

bool get isCustomClient {
  _isCustomClient ??= bind.isCustomClient();
  return _isCustomClient!;
}

get defaultOptionLang => isCustomClient ? 'default' : '';

get defaultOptionTheme => isCustomClient ? 'system' : '';

get defaultOptionYes => isCustomClient ? 'Y' : '';

get defaultOptionNo => isCustomClient ? 'N' : '';

get defaultOptionWhitelist => isCustomClient ? ',' : '';

get defaultOptionAccessMode => isCustomClient ? 'custom' : '';

get defaultOptionApproveMode => isCustomClient ? 'password-click' : '';

bool whitelistNotEmpty() {
  // https://rustdesk.com/docs/en/self-host/client-configuration/advanced-settings/#whitelist
  final v = bind.mainGetOptionSync(key: kOptionWhitelist);
  return v != '' && v != ',';
}

bool idWhitelistNotEmpty() {
  final v = bind.mainGetOptionSync(key: kOptionIdWhitelist);
  return v != '' && v != ',';
}

void checkUpdate() {
  if (!bind.isCustomClient()) {
    platformFFI.registerEventHandler(
        kCheckSoftwareUpdateFinish, kCheckSoftwareUpdateFinish,
        (Map<String, dynamic> evt) async {
      if (evt['url'] is String) {
        stateGlobal.updateUrl.value = evt['url'];
      }
    });
    Timer(const Duration(seconds: 1), () async {
      bind.mainGetSoftwareUpdateUrl();
    });
  }
}

String _appName = '';

String get appName {
  if (_appName.isEmpty) {
    _appName = 'OpenUU';
  }
  return _appName;
}
