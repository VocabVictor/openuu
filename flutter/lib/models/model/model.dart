import 'dart:async';
import '../quick_launch_model.dart';
import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:bot_toast/bot_toast.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/common/window_fit.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/cm_file_model.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/group_model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/models/user_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/common/widgets/ui_dialog.dart';
import 'package:flutter_hbb/common/widgets/ui_fields.dart';
import 'package:window_size/window_size.dart' as window_size;
import 'package:flutter_hbb/models/desktop_render_texture.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/utils/http_service.dart' as http;
import 'package:tuple/tuple.dart';
import 'package:image/image.dart' as img2;
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:uuid/uuid.dart';
import 'package:window_manager/window_manager.dart';
import 'package:file_picker/file_picker.dart';
import 'package:vector_math/vector_math.dart' show Vector2;

import '../../common.dart';
import '../../utils/image.dart' as img;
import '../../common/widgets/dialog.dart';
import '../input_model.dart';
import '../platform_model.dart';
import 'package:flutter_hbb/utils/scale.dart';

import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';

part 'canvas_model.dart';
part 'canvas_model_layout.dart';
part 'canvas_model_pan.dart';
part 'canvas_model_scroll.dart';
part 'cursor_data.dart';
part 'cursor_model.dart';
part 'cursor_model_data.dart';
part 'cursor_model_pan.dart';
part 'cursor_model_position.dart';
part 'ffi.dart';
part 'ffi_model_display.dart';
part 'ffi_model_displays.dart';
part 'ffi_model_listener.dart';
part 'ffi_model_msgbox.dart';
part 'ffi_model_peer_info.dart';
part 'ffi_model_window_fit.dart';
part 'ffi_model_reconnect.dart';
part 'ffi_model_state.dart';
part 'ffi_model_sync.dart';
part 'ffi_session.dart';
part 'ffi_start.dart';
part 'image_model.dart';
part 'peer_info.dart';
part 'quality_model.dart';
part 'view_style.dart';

typedef HandleMsgBox = Function(Map<String, dynamic> evt, String id);

typedef ReconnectHandle = Function(OverlayDialogManager, SessionID, bool);

final _constSessionId = Uuid().v4obj();

// Empirical restart reconnect cadence: keep the last frame briefly and retry quickly.
const _restartReconnectSilentDelaySecs = 5;

class CachedPeerData {
  Map<String, dynamic> updatePrivacyMode = {};
  Map<String, dynamic> peerInfo = {};
  List<Map<String, dynamic>> cursorDataList = [];
  Map<String, dynamic> lastCursorId = {};
  Map<String, bool> permissions = {};

  bool secure = false;
  bool direct = false;
  String streamType = '';

  CachedPeerData();

  @override
  String toString() {
    return jsonEncode({
      'updatePrivacyMode': updatePrivacyMode,
      'peerInfo': peerInfo,
      'cursorDataList': cursorDataList,
      'lastCursorId': lastCursorId,
      'permissions': permissions,
      'secure': secure,
      'direct': direct,
      'streamType': streamType,
    });
  }

  static CachedPeerData? fromString(String s) {
    try {
      final map = jsonDecode(s);
      final data = CachedPeerData();
      data.updatePrivacyMode = map['updatePrivacyMode'];
      data.peerInfo = map['peerInfo'];
      for (final cursorData in map['cursorDataList']) {
        data.cursorDataList.add(cursorData);
      }
      data.lastCursorId = map['lastCursorId'];
      map['permissions'].forEach((key, value) {
        data.permissions[key] = value;
      });
      data.secure = map['secure'];
      data.direct = map['direct'];
      data.streamType = map['streamType'];
      return data;
    } catch (e) {
      debugPrint('Failed to parse CachedPeerData: $e');
      return null;
    }
  }
}

class FfiModel with ChangeNotifier {
  void _notify() => notifyListeners();
  CachedPeerData cachedPeerData = CachedPeerData();
  PeerInfo _pi = PeerInfo();
  int? lastUserDisplay;
  int? pendingMonitorRestore;
  Timer? _pendingRestoreTimer;
  Rect? _rect;

  var _inputBlocked = false;
  final _permissions = <String, bool>{};
  bool? _secure;
  bool? _direct;
  bool _touchMode = false;
  late VirtualMouseMode virtualMouseMode;
  Timer? _timer;
  Timer? _restartReconnectDelayTimer;
  var _reconnects = 1;
  DateTime? _offlineReconnectStartTime;
  bool _androidDocumentPickerActive = false;
  bool _androidDocumentPickerInterruptedConnection = false;
  bool _viewOnly = false;
  bool _showMyCursor = false;
  WeakReference<FFI> parent;
  late final SessionID sessionId;

  RxBool waitForImageDialogShow = true.obs;
  Timer? waitForImageTimer;
  RxBool waitForFirstImage = true.obs;
  bool isRefreshing = false;

  Timer? timerScreenshot;

  Rect? get rect => _rect;
  bool get isOriginalResolutionSet =>
      _pi.tryGetDisplayIfNotAllDisplay()?.isOriginalResolutionSet ?? false;
  bool get isVirtualDisplayResolution =>
      _pi.tryGetDisplayIfNotAllDisplay()?.isVirtualDisplayResolution ?? false;
  bool get isOriginalResolution =>
      _pi.tryGetDisplayIfNotAllDisplay()?.isOriginalResolution ?? false;

  Map<String, bool> get permissions => _permissions;
  setPermissions(Map<String, bool> permissions) {
    _permissions.clear();
    _permissions.addAll(permissions);
  }

  bool? get secure => _secure;

  bool? get direct => _direct;

  PeerInfo get pi => _pi;

  bool get inputBlocked => _inputBlocked;

  bool get touchMode => _touchMode;

  bool get isPeerAndroid => _pi.platform == kPeerPlatformAndroid;
  bool get isPeerMobile => isPeerAndroid;

  bool get isPeerLinux => _pi.platform == kPeerPlatformLinux;

  bool get viewOnly => _viewOnly;
  bool get showMyCursor => _showMyCursor;

  set inputBlocked(v) {
    _inputBlocked = v;
  }

  FfiModel(this.parent) {
    clear();
    sessionId = parent.target!.sessionId;
    cachedPeerData.permissions = _permissions;
    virtualMouseMode = VirtualMouseMode(this);
  }

}
