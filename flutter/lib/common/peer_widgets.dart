import 'dart:convert';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:get/get.dart';

import '../../consts.dart';
import '../models/model.dart';
import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'globals.dart';

String getDesktopTabLabel(String peerId, String alias) {
  String label = alias.isEmpty ? peerId : alias;
  try {
    String peer = bind.mainGetPeerSync(id: peerId);
    Map<String, dynamic> config = jsonDecode(peer);
    if (config['info']['hostname'] is String) {
      String hostname = config['info']['hostname'];
      if (hostname.isNotEmpty &&
          !label.toLowerCase().contains(hostname.toLowerCase())) {
        label += "@$hostname";
      }
    }
  } catch (e) {
    debugPrint("Failed to get hostname:$e");
  }
  return label;
}

sessionRefreshVideo(SessionID sessionId, PeerInfo pi) async {
  if (pi.currentDisplay == kAllDisplayValue) {
    for (int i = 0; i < pi.displays.length; i++) {
      await bind.sessionRefresh(sessionId: sessionId, display: i);
    }
  } else {
    await bind.sessionRefresh(sessionId: sessionId, display: pi.currentDisplay);
  }
}

Widget netWorkErrorWidget() {
  return Center(
      child: Column(
    mainAxisAlignment: MainAxisAlignment.center,
    crossAxisAlignment: CrossAxisAlignment.center,
    children: [
      if (!gFFI.userModel.networkErrorFromServer.value)
        Text(translate("network_error_tip")),
      ElevatedButton(
              onPressed: gFFI.userModel.refreshCurrentUser,
              child: Text(translate("Retry")))
          .marginSymmetric(vertical: 16),
      SelectableText(gFFI.userModel.networkError.value,
          style: TextStyle(fontSize: 11, color: Colors.red)),
    ],
  ));
}

void updateTextAndPreserveSelection(
    TextEditingController controller, String text) {
  // Only care about select all for now.
  final isSelected = controller.selection.isValid &&
      controller.selection.end > controller.selection.start;

  // Set text will make the selection invalid.
  controller.text = text;

  if (isSelected) {
    controller.selection = TextSelection(
        baseOffset: 0, extentOffset: controller.value.text.length);
  }
}

String getConnectionText(bool secure, bool direct, String streamType) {
  String connectionText;
  if (secure && direct) {
    connectionText = translate("Direct and encrypted connection");
  } else if (secure && !direct) {
    connectionText = translate("Relayed and encrypted connection");
  } else if (!secure && direct) {
    connectionText = translate("Direct and unencrypted connection");
  } else {
    connectionText = translate("Relayed and unencrypted connection");
  }
  if (streamType == 'Relay') {
    streamType = 'TCP';
  }
  if (streamType.isEmpty) {
    return connectionText;
  } else {
    return '$connectionText ($streamType)';
  }
}

bool peerTabShowNote(PeerTabIndex peerTabIndex) {
  return peerTabIndex == PeerTabIndex.ab || peerTabIndex == PeerTabIndex.group;
}

// TODO: We should support individual bits combinations in the future.
// But for now, just keep it simple, because the old code only supports single button.
// No users have requested multi-button support yet.
String mouseButtonsToPeer(int buttons) {
  switch (buttons) {
    case kPrimaryMouseButton:
      return 'left';
    case kSecondaryMouseButton:
      return 'right';
    case kMiddleMouseButton:
      return 'wheel';
    case kBackMouseButton:
      return 'back';
    case kForwardMouseButton:
      return 'forward';
    default:
      return '';
  }
}

/// Build an avatar widget from an avatar URL or data URI string.
/// Returns [fallback] if avatar is empty or cannot be decoded.
/// [borderRadius] defaults to [size]/2 (circle).
Widget? buildAvatarWidget({
  required String avatar,
  required double size,
  double? borderRadius,
  Widget? fallback,
}) {
  final trimmed = avatar.trim();
  if (trimmed.isEmpty) return fallback;

  ImageProvider? imageProvider;
  if (trimmed.startsWith('data:image/')) {
    final comma = trimmed.indexOf(',');
    if (comma > 0) {
      try {
        imageProvider = MemoryImage(base64Decode(trimmed.substring(comma + 1)));
      } catch (_) {}
    }
  } else if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    imageProvider = NetworkImage(trimmed);
  }

  if (imageProvider == null) return fallback;

  final radius = borderRadius ?? size / 2;
  return ClipRRect(
    borderRadius: BorderRadius.circular(radius),
    child: Image(
      image: imageProvider,
      width: size,
      height: size,
      fit: BoxFit.cover,
      errorBuilder: (_, __, ___) => fallback ?? SizedBox.shrink(),
    ),
  );
}
