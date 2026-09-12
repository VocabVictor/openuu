part of 'model.dart';

extension FfiModelState on FfiModel {
  Rect? globalDisplaysRect() => _getDisplaysRect(_pi.displays, true);
  Rect? displaysRect() => _getDisplaysRect(_pi.getCurDisplays(), false);
  Rect? _getDisplaysRect(List<Display> displays, bool useDisplayScale) {
    if (displays.isEmpty) {
      return null;
    }
    if (isPeerLinux) {
      useDisplayScale = true;
    }
    int scale(int len, double s) {
      if (useDisplayScale) {
        return len.toDouble() ~/ s;
      } else {
        return len;
      }
    }

    double l = displays[0].x;
    double t = displays[0].y;
    double r = displays[0].x + scale(displays[0].width, displays[0].scale);
    double b = displays[0].y + scale(displays[0].height, displays[0].scale);
    for (var display in displays.sublist(1)) {
      l = min(l, display.x);
      t = min(t, display.y);
      r = max(r, display.x + scale(display.width, display.scale));
      b = max(b, display.y + scale(display.height, display.scale));
    }
    return Rect.fromLTRB(l, t, r, b);
  }

  toggleTouchMode() {
    if (!isPeerAndroid) {
      _touchMode = !_touchMode;
      _notify();
    }
  }

  updatePermission(Map<String, dynamic> evt, String id) {
    // Track previous keyboard permission to detect revocation.
    final hadKeyboardPerm = _permissions['keyboard'] != false;

    evt.forEach((k, v) {
      if (k == 'name' || k.isEmpty) return;
      _permissions[k] = v == 'true';
    });
    // Only inited at remote page
    if (parent.target?.connType == ConnType.defaultConn) {
      KeyboardEnabledState.find(id).value = _permissions['keyboard'] != false;
    }

    // If keyboard permission was revoked while relative mouse mode is active,
    // forcefully disable relative mouse mode to prevent the user from being trapped.
    final hasKeyboardPerm = _permissions['keyboard'] != false;
    if (hadKeyboardPerm && !hasKeyboardPerm) {
      final inputModel = parent.target?.inputModel;
      if (inputModel != null && inputModel.relativeMouseMode.value) {
        inputModel.setRelativeMouseMode(false);
        showToast(translate('rel-mouse-permission-lost-tip'));
      }
    }

    debugPrint('updatePermission: $_permissions');
    _notify();
  }

  bool get keyboard => _permissions['keyboard'] != false;

  clear() {
    _pi = PeerInfo();
    lastUserDisplay = null;
    _cancelPendingMonitorRestore();
    _secure = null;
    _direct = null;
    _inputBlocked = false;
    _timer?.cancel();
    _timer = null;
    _androidDocumentPickerActive = false;
    _androidDocumentPickerInterruptedConnection = false;
    resetRestartReconnectState();
    clearPermissions();
    waitForImageTimer?.cancel();
    timerScreenshot?.cancel();
  }

  setConnectionType(
      String peerId, bool secure, bool direct, String streamType) {
    cachedPeerData.secure = secure;
    cachedPeerData.direct = direct;
    cachedPeerData.streamType = streamType;
    _secure = secure;
    _direct = direct;
    try {
      var connectionType = ConnectionTypeState.find(peerId);
      connectionType.setSecure(secure);
      connectionType.setDirect(direct);
      connectionType.setStreamType(streamType);
    } catch (e) {
      //
    }
  }

  Widget? getConnectionImageText() {
    if (secure == null || direct == null) {
      return null;
    } else {
      final icon =
          '${secure == true ? 'secure' : 'insecure'}${direct == true ? '' : '_relay'}';
      final iconWidget =
          SvgPicture.asset('assets/$icon.svg', width: 48, height: 48);
      String connectionText =
          getConnectionText(secure!, direct!, cachedPeerData.streamType);
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          iconWidget,
          SizedBox(height: 4),
          Text(
            connectionText,
            style: TextStyle(fontSize: 12),
            textAlign: TextAlign.center,
          ),
        ],
      );
    }
  }

  clearPermissions() {
    _inputBlocked = false;
    _permissions.clear();
  }

  handleCachedPeerData(CachedPeerData data, String peerId) async {
    handleMsgBox({
      'type': 'success',
      'title': 'Successful',
      'text': kMsgboxTextWaitingForImage,
      'link': '',
    }, sessionId, peerId);
    updatePrivacyMode(data.updatePrivacyMode, sessionId, peerId);
    setConnectionType(peerId, data.secure, data.direct, data.streamType);
    await handlePeerInfo(data.peerInfo, peerId, true);
    for (final element in data.cursorDataList) {
      updateLastCursorId(element);
      await handleCursorData(element);
    }
    if (data.lastCursorId.isNotEmpty) {
      updateLastCursorId(data.lastCursorId);
      handleCursorId(data.lastCursorId);
    }
  }
}
