part of 'input_model.dart';

extension InputModelKeyEvents on InputModel {
  KeyEventResult handleRawKeyEvent(RawKeyEvent e) {
    if (isViewOnly) return KeyEventResult.handled;
    if (isViewCamera) return KeyEventResult.handled;
    if (!isInputSourceFlutter) {
      if (isDesktop) {
        return KeyEventResult.handled;
      } else if (isWeb) {
        return KeyEventResult.ignored;
      }
    }

    if (_relativeMouse.handleRawKeyEvent(e)) {
      return KeyEventResult.handled;
    }

    bool iosCapsLock = false;
    if (isIOS && e is RawKeyDownEvent) {
      iosCapsLock = _getIosCapsFromRawCharacter(e);
    }

    final key = e.logicalKey;
    if (e is RawKeyDownEvent) {
      if (!e.repeat) {
        if (e.isAltPressed && !alt) {
          alt = true;
        } else if (e.isControlPressed && !ctrl) {
          ctrl = true;
        } else if (e.isShiftPressed && !shift) {
          shift = true;
        } else if (e.isMetaPressed && !command) {
          command = true;
        }
      }
      toReleaseRawKeys.updateKeyDown(key, e);
    }
    if (e is RawKeyUpEvent) {
      if (key == LogicalKeyboardKey.altLeft ||
          key == LogicalKeyboardKey.altRight) {
        alt = false;
      } else if (key == LogicalKeyboardKey.controlLeft ||
          key == LogicalKeyboardKey.controlRight) {
        ctrl = false;
      } else if (key == LogicalKeyboardKey.shiftRight ||
          key == LogicalKeyboardKey.shiftLeft) {
        shift = false;
      } else if (key == LogicalKeyboardKey.metaLeft ||
          key == LogicalKeyboardKey.metaRight ||
          key == LogicalKeyboardKey.superKey) {
        command = false;
      }

      toReleaseRawKeys.updateKeyUp(key, e);
    }

    // On some mobile soft-keyboard paths, Flutter may leave cached Shift state
    // set even though the current raw key event is not shifted anymore.
    if (e is RawKeyDownEvent &&
        shouldReleaseStaleMobileShift(
          isMobile: isMobile,
          cachedShiftPressed: shift,
          actualShiftPressed: e.isShiftPressed,
          logicalKey: e.logicalKey,
          hasTrackedShiftKeyDown: toReleaseRawKeys.lastLShiftKeyEvent != null ||
              toReleaseRawKeys.lastRShiftKeyEvent != null,
        )) {
      if (kDebugMode) {
        debugPrint(
          'input: releasing stale mobile Shift before replaying tracked raw '
          'key-up (logicalKey=${e.logicalKey.keyLabel}, '
          'actualShiftPressed=${e.isShiftPressed}, cachedShiftPressed=$shift)',
        );
      }
      _releaseTrackedRawShiftKeyEventIfNeeded();
    }

    // * Currently mobile does not enable map mode
    if ((isDesktop || isWebDesktop) && keyboardMode == kKeyMapMode) {
      mapKeyboardModeRaw(e, iosCapsLock);
    } else {
      legacyKeyboardModeRaw(e);
    }

    return KeyEventResult.handled;
  }

  KeyEventResult handleKeyEvent(KeyEvent e) {
    if (isViewOnly) return KeyEventResult.handled;
    if (isViewCamera) return KeyEventResult.handled;
    if (!isInputSourceFlutter) {
      if (isDesktop) {
        return KeyEventResult.handled;
      } else if (isWeb) {
        return KeyEventResult.ignored;
      }
    }
    if (isWindows || isLinux) {
      // Ignore meta keys. Because flutter window will loose focus if meta key is pressed.
      if (e.physicalKey == PhysicalKeyboardKey.metaLeft ||
          e.physicalKey == PhysicalKeyboardKey.metaRight) {
        return KeyEventResult.handled;
      }
    }

    if (_relativeMouse.handleKeyEvent(
      e,
      ctrlPressed: ctrl,
      shiftPressed: shift,
      altPressed: alt,
      commandPressed: command,
    )) {
      return KeyEventResult.handled;
    }

    bool iosCapsLock = false;
    if (isIOS && (e is KeyDownEvent || e is KeyRepeatEvent)) {
      iosCapsLock = _getIosCapsFromCharacter(e);
    }

    // Update cached modifier state before sending the event. The stale mobile
    // Shift release check below relies on this cached state.
    if (e is KeyUpEvent) {
      handleKeyUpEventModifiers(e);
    } else if (e is KeyDownEvent) {
      handleKeyDownEventModifiers(e);
    }

    bool isMobileAndMapMode = false;
    if (isMobile) {
      // Do not use map mode if mobile -> Android. Android does not support map mode for now.
      // Because simulating the physical key events(uhid) which requires root permission is not supported.
      if (peerPlatform != kPeerPlatformAndroid) {
        if (isIOS) {
          isMobileAndMapMode = true;
        } else {
          // The physicalKey.usbHidUsage may be not correct for soft keyboard on Android.
          // iOS does not have this issue.
          // 1. Open the soft keyboard on Android
          // 2. Switch to input method like zh/ko/ja
          // 3. Click Backspace and Enter on the soft keyboard or physical keyboard
          // 4. The physicalKey.usbHidUsage is not correct.
          // PhysicalKeyboardKey#8ac83(usbHidUsage: "0x1100000042", debugName: "Key with ID 0x1100000042")
          // LogicalKeyboardKey#2604c(keyId: "0x10000000d", keyLabel: "Enter", debugName: "Enter")
          //
          // The correct PhysicalKeyboardKey should be
          // PhysicalKeyboardKey#e14a9(usbHidUsage: "0x00070028", debugName: "Enter")
          // https://github.com/flutter/flutter/issues/157771
          // We cannot use the debugName to determine the key is correct or not, because it's null in release mode.
          // The normal `usbHidUsage` for keyboard shoud be between [0x00000010, 0x000c029f]
          // https://github.com/flutter/flutter/blob/c051b69e2a2224300e20d93dbd15f4b91e8844d1/packages/flutter/lib/src/services/keyboard_key.g.dart#L5332 - 5600
          final isNormalHsbHidUsage = (e.physicalKey.usbHidUsage >> 20) == 0;
          isMobileAndMapMode = isNormalHsbHidUsage &&
              // No need to check `!['Backspace', 'Enter'].contains(e.logicalKey.keyLabel)`
              // But we still add it for more reliability.
              !['Backspace', 'Enter'].contains(e.logicalKey.keyLabel);
        }
      }
    }

    // On some mobile soft-keyboard paths, Flutter may leave cached Shift state
    // set even though the current key event is not shifted anymore.
    if (e is KeyDownEvent &&
        shouldReleaseStaleMobileShift(
          isMobile: isMobile,
          cachedShiftPressed: shift,
          actualShiftPressed: HardwareKeyboard.instance.isShiftPressed,
          logicalKey: e.logicalKey,
          hasTrackedShiftKeyDown: toReleaseKeys.lastLShiftKeyEvent != null ||
              toReleaseKeys.lastRShiftKeyEvent != null,
        )) {
      _releaseTrackedShiftKeyEventIfNeeded();
    }

    final isDesktopAndMapMode =
        isDesktop || (isWebDesktop && keyboardMode == kKeyMapMode);
    if (isMobileAndMapMode || isDesktopAndMapMode) {
      // FIXME: e.character is wrong for dead keys, eg: ^ in de
      newKeyboardMode(
          e.character ?? '',
          e.physicalKey.usbHidUsage & 0xFFFF,
          // Show repeat event be converted to "release+press" events?
          e is KeyDownEvent || e is KeyRepeatEvent,
          iosCapsLock);
    } else {
      legacyKeyboardMode(e);
    }

    return KeyEventResult.handled;
  }
}
