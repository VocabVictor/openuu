part of 'model.dart';

extension FfiModelDisplays on FfiModel {
  tryUseAllMyDisplaysForTheRemoteSession(String peerId) async {
    if (bind.sessionGetUseAllMyDisplaysForTheRemoteSession(
            sessionId: sessionId) !=
        'Y') {
      return;
    }

    if (!_pi.isSupportMultiDisplay || _pi.displays.length <= 1) {
      return;
    }

    final screenRectList = await getScreenRectList();
    if (screenRectList.length <= 1) {
      return;
    }

    // to-do: peer currentDisplay is the primary display, but the primary display may not be the first display.
    // local primary display also may not be the first display.
    //
    // 0 is assumed to be the primary display here, for now.

    // move to the first display and set fullscreen
    bind.sessionSwitchDisplay(
      isDesktop: isDesktop,
      sessionId: sessionId,
      value: Int32List.fromList([0]),
    );
    _pi.currentDisplay = 0;
    try {
      CurrentDisplayState.find(peerId).value = _pi.currentDisplay;
    } catch (e) {
      //
    }
    await tryMoveToScreenAndSetFullscreen(screenRectList[0]);

    final length = _pi.displays.length < screenRectList.length
        ? _pi.displays.length
        : screenRectList.length;
    for (var i = 1; i < length; i++) {
      openMonitorInNewTabOrWindow(i, peerId, _pi,
          screenRect: screenRectList[i]);
    }
  }

  tryShowAndroidActionsOverlay({int delayMSecs = 10}) {
    if (isPeerAndroid) {
      if (parent.target?.connType == ConnType.defaultConn &&
          parent.target != null &&
          parent.target!.ffiModel.permissions['keyboard'] != false) {
        Timer(Duration(milliseconds: delayMSecs), () {
          if (parent.target!.dialogManager.mobileActionsOverlayVisible.isTrue) {
            parent.target!.dialogManager
                .showMobileActionsOverlay(ffi: parent.target!);
          }
        });
      }
    }
  }

  handleResolutions(String id, dynamic resolutions) {
    try {
      final resolutionsObj = json.decode(resolutions as String);
      late List<dynamic> dynamicArray;
      if (resolutionsObj is Map) {
        // The web version
        dynamicArray = (resolutionsObj as Map<String, dynamic>)['resolutions']
            as List<dynamic>;
      } else {
        // The rust version
        dynamicArray = resolutionsObj as List<dynamic>;
      }
      List<Resolution> arr = List.empty(growable: true);
      for (int i = 0; i < dynamicArray.length; i++) {
        var width = dynamicArray[i]["width"];
        var height = dynamicArray[i]["height"];
        if (width is int && width > 0 && height is int && height > 0) {
          arr.add(Resolution(width, height));
        }
      }
      arr.sort((a, b) {
        if (b.width != a.width) {
          return b.width - a.width;
        } else {
          return b.height - a.height;
        }
      });
      _pi.resolutions = arr;
    } catch (e) {
      debugPrint("Failed to parse resolutions:$e");
    }
  }

  Display evtToDisplay(Map<String, dynamic> evt) {
    var d = Display();
    d.x = evt['x']?.toDouble() ?? d.x;
    d.y = evt['y']?.toDouble() ?? d.y;
    d.width = evt['width'] ?? d.width;
    d.height = evt['height'] ?? d.height;
    d.cursorEmbedded = evt['cursor_embedded'] == 1;
    d.originalWidth = evt['original_width'] ?? kInvalidResolutionValue;
    d.originalHeight = evt['original_height'] ?? kInvalidResolutionValue;
    d._scale = 1.0;
    final scaledWidth = evt['scaled_width'];
    if (scaledWidth != null) {
      final sw = int.tryParse(scaledWidth.toString());
      if (sw != null && sw > 0 && d.width > 0) {
        d._scale = max(d.width.toDouble() / sw, 1.0);
      } else {
        debugPrint(
            "Invalid scaled_width ($scaledWidth) or width (${d.width}), using default scale 1.0");
      }
    }
    return d;
  }

  updateLastCursorId(Map<String, dynamic> evt) {
    // int.parse(evt['id']) may cause FormatException
    // Unhandled Exception: FormatException: Positive input exceeds the limit of integer 18446744071749110741
    parent.target?.cursorModel.id = evt['id'];
  }

  handleCursorId(Map<String, dynamic> evt) {
    cachedPeerData.lastCursorId = evt;
    parent.target?.cursorModel.updateCursorId(evt);
  }

  handleCursorData(Map<String, dynamic> evt) async {
    cachedPeerData.cursorDataList.add(evt);
    await parent.target?.cursorModel.updateCursorData(evt);
  }
}
