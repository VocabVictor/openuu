part of 'input_model.dart';

extension InputModelKeyboard on InputModel {
  // https://github.com/flutter/flutter/issues/157241
  // Infer CapsLock state from the character output.
  // This is needed because Flutter's HardwareKeyboard.lockModesEnabled may report
  // incorrect CapsLock state on iOS.
  bool _getIosCapsFromCharacter(KeyEvent e) {
    if (!isIOS) return false;
    final ch = e.character;
    return _getIosCapsFromCharacterImpl(
        ch, HardwareKeyboard.instance.isShiftPressed);
  }

  // RawKeyEvent version of _getIosCapsFromCharacter.
  bool _getIosCapsFromRawCharacter(RawKeyEvent e) {
    if (!isIOS) return false;
    final ch = e.character;
    return _getIosCapsFromCharacterImpl(ch, e.isShiftPressed);
  }

  // Shared implementation for inferring CapsLock state from character.
  // Uses Unicode-aware case detection to support non-ASCII letters (e.g., ü/Ü, é/É).
  //
  // Limitations:
  // 1. This inference assumes the client and server use the same keyboard layout.
  //    If layouts differ (e.g., client uses EN, server uses DE), the character output
  //    may not match expectations. For example, ';' on EN layout maps to 'ö' on DE
  //    layout, making it impossible to correctly infer CapsLock state from the
  //    character alone.
  // 2. On iOS, CapsLock+Shift produces uppercase letters (unlike desktop where it
  //    produces lowercase). This method cannot handle that case correctly.
  bool _getIosCapsFromCharacterImpl(String? ch, bool shiftPressed) {
    if (ch == null || ch.length != 1) return false;
    // Use Dart's built-in Unicode-aware case detection
    final upper = ch.toUpperCase();
    final lower = ch.toLowerCase();
    final isUpper = upper == ch && lower != ch;
    final isLower = lower == ch && upper != ch;
    // Skip non-letter characters (e.g., numbers, symbols, CJK characters without case)
    if (!isUpper && !isLower) return false;
    return isUpper != shiftPressed;
  }

  int _buildLockModes(bool iosCapsLock) {
    const capslock = 1;
    const numlock = 2;
    const scrolllock = 3;
    int lockModes = 0;
    if (isIOS) {
      if (iosCapsLock) {
        lockModes |= (1 << capslock);
      }
      // Ignore "NumLock/ScrollLock" on iOS for now.
    } else {
      if (HardwareKeyboard.instance.lockModesEnabled
          .contains(KeyboardLockMode.capsLock)) {
        lockModes |= (1 << capslock);
      }
      if (HardwareKeyboard.instance.lockModesEnabled
          .contains(KeyboardLockMode.numLock)) {
        lockModes |= (1 << numlock);
      }
      if (HardwareKeyboard.instance.lockModesEnabled
          .contains(KeyboardLockMode.scrollLock)) {
        lockModes |= (1 << scrolllock);
      }
    }
    return lockModes;
  }

  // This function must be called after the peer info is received.
  // Because `sessionGetKeyboardMode` relies on the peer version.
  updateKeyboardMode() async {
    // * Currently mobile does not enable map mode
    if (isDesktop) {
      keyboardMode = await bind.sessionGetKeyboardMode(sessionId: sessionId) ??
          kKeyLegacyMode;
    }
  }

  /// Updates the trackpad speed based on the session value.
  ///
  /// The expected format of the retrieved value is a string that can be parsed into a double.
  /// If parsing fails or the value is out of bounds (less than `kMinTrackpadSpeed` or greater
  /// than `kMaxTrackpadSpeed`), the trackpad speed is reset to the default
  /// value (`kDefaultTrackpadSpeed`).
  ///
  /// Bounds:
  /// - Minimum: `kMinTrackpadSpeed`
  /// - Maximum: `kMaxTrackpadSpeed`
  /// - Default: `kDefaultTrackpadSpeed`
  Future<void> updateTrackpadSpeed() async {
    _trackpadSpeed =
        (await bind.sessionGetTrackpadSpeed(sessionId: sessionId) ??
            kDefaultTrackpadSpeed);
    if (_trackpadSpeed < kMinTrackpadSpeed ||
        _trackpadSpeed > kMaxTrackpadSpeed) {
      _trackpadSpeed = kDefaultTrackpadSpeed;
    }
    _trackpadSpeedInner = _trackpadSpeed / 100.0;
  }

  void handleKeyDownEventModifiers(KeyEvent e) {
    KeyUpEvent upEvent(e) => KeyUpEvent(
          physicalKey: e.physicalKey,
          logicalKey: e.logicalKey,
          timeStamp: e.timeStamp,
        );
    if (e.logicalKey == LogicalKeyboardKey.altLeft) {
      if (!alt) {
        alt = true;
      }
      toReleaseKeys.lastLAltKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.altRight) {
      if (!alt) {
        alt = true;
      }
      toReleaseKeys.lastLAltKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.controlLeft) {
      if (!ctrl) {
        ctrl = true;
      }
      toReleaseKeys.lastLCtrlKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.controlRight) {
      if (!ctrl) {
        ctrl = true;
      }
      toReleaseKeys.lastRCtrlKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.shiftLeft) {
      if (!shift) {
        shift = true;
      }
      toReleaseKeys.lastLShiftKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.shiftRight) {
      if (!shift) {
        shift = true;
      }
      toReleaseKeys.lastRShiftKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.metaLeft) {
      if (!command) {
        command = true;
      }
      toReleaseKeys.lastLCommandKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.metaRight) {
      if (!command) {
        command = true;
      }
      toReleaseKeys.lastRCommandKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.superKey) {
      if (!command) {
        command = true;
      }
      toReleaseKeys.lastSuperKeyEvent = upEvent(e);
    }
  }

  void handleKeyUpEventModifiers(KeyEvent e) {
    if (e.logicalKey == LogicalKeyboardKey.altLeft) {
      alt = false;
      toReleaseKeys.lastLAltKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.altRight) {
      alt = false;
      toReleaseKeys.lastRAltKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.controlLeft) {
      ctrl = false;
      toReleaseKeys.lastLCtrlKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.controlRight) {
      ctrl = false;
      toReleaseKeys.lastRCtrlKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.shiftLeft) {
      shift = false;
      toReleaseKeys.lastLShiftKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.shiftRight) {
      shift = false;
      toReleaseKeys.lastRShiftKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.metaLeft) {
      command = false;
      toReleaseKeys.lastLCommandKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.metaRight) {
      command = false;
      toReleaseKeys.lastRCommandKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.superKey) {
      command = false;
      toReleaseKeys.lastSuperKeyEvent = null;
    }
  }

  // Safe: this only re-dispatches synthesized Shift key-up events.
  // The key-up path clears the tracked Shift state so this does not loop.
  void _releaseTrackedShiftKeyEventIfNeeded() {
    final leftShift = toReleaseKeys.lastLShiftKeyEvent;
    final rightShift = toReleaseKeys.lastRShiftKeyEvent;
    if (leftShift != null) {
      handleKeyEvent(leftShift);
    }
    if (rightShift != null) {
      handleKeyEvent(rightShift);
    }
  }

  // Safe: this only re-dispatches synthesized Shift key-up events.
  // The raw key-up path clears the tracked Shift state so this does not loop.
  void _releaseTrackedRawShiftKeyEventIfNeeded() {
    final leftShift = toReleaseRawKeys.lastLShiftKeyEvent;
    final rightShift = toReleaseRawKeys.lastRShiftKeyEvent;
    if (leftShift != null) {
      handleRawKeyEvent(RawKeyUpEvent(
        data: leftShift.data,
        character: leftShift.character,
      ));
    }
    if (rightShift != null) {
      handleRawKeyEvent(RawKeyUpEvent(
        data: rightShift.data,
        character: rightShift.character,
      ));
    }
  }
}
