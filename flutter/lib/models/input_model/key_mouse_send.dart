part of 'input_model.dart';

extension InputModelMouseSend on InputModel {
  Map<String, dynamic> _getMouseEvent(PointerEvent evt, String type) {
    final Map<String, dynamic> out = {};

    bool hasStaleButtonsOnMouseUp =
        type == _kMouseEventUp && evt.buttons == _lastButtons;

    // Check update event type and set buttons to be sent.
    int buttons = _lastButtons;
    if (type == _kMouseEventMove) {
      // flutter may emit move event if one button is pressed and another button
      // is pressing or releasing.
      if (evt.buttons != _lastButtons) {
        // For simplicity
        // Just consider 3 - 1 ((Left + Right buttons) - Left button)
        // Do not consider 2 - 1 (Right button - Left button)
        // or 6 - 5 ((Right + Mid buttons) - (Left + Mid buttons))
        // and so on
        buttons = evt.buttons - _lastButtons;
        if (buttons > 0) {
          type = _kMouseEventDown;
        } else {
          type = _kMouseEventUp;
          buttons = -buttons;
        }
      }
    } else {
      if (evt.buttons != 0) {
        buttons = evt.buttons;
      }
    }
    _lastButtons = hasStaleButtonsOnMouseUp ? 0 : evt.buttons;

    out['buttons'] = buttons;
    out['type'] = type;
    return out;
  }

  /// Send a mouse tap event(down and up).
  Future<void> tap(MouseButtons button) async {
    await sendMouse('down', button);
    await sendMouse('up', button);
  }

  Future<void> tapDown(MouseButtons button) async {
    await sendMouse('down', button);
  }

  Future<void> tapUp(MouseButtons button) async {
    await sendMouse('up', button);
  }

  /// Send scroll event with scroll distance [y].
  Future<void> scroll(int y) async {
    if (isViewCamera) return;
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json
            .encode(modify({'id': id, 'type': 'wheel', 'y': y.toString()})));
  }

  /// Reset key modifiers to false, including [shift], [ctrl], [alt] and [command].
  void resetModifiers() {
    shift = ctrl = alt = command = false;
  }

  /// Modify the given modifier map [evt] based on current modifier key status.
  Map<String, dynamic> modify(Map<String, dynamic> evt) {
    if (ctrl) evt['ctrl'] = 'true';
    if (shift) evt['shift'] = 'true';
    if (alt) evt['alt'] = 'true';
    if (command) evt['command'] = 'true';
    return evt;
  }

  /// Send mouse event unconditionally (no permission checks).
  /// Used for side button releases that must go through even if permissions
  /// changed after the matching down was sent.
  Future<void> _sendMouseUnchecked(String type, MouseButtons button) async {
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify({'type': type, 'buttons': button.value})));
  }

  /// Send mouse press event.
  Future<void> sendMouse(String type, MouseButtons button) async {
    if (!keyboardPerm) return;
    if (isViewCamera) return;
    await _sendMouseUnchecked(type, button);
  }
}

extension InputModelKeySend on InputModel {
  /// Send Key Event
  void newKeyboardMode(
      String character, int usbHid, bool down, bool iosCapsLock) {
    final lockModes = _buildLockModes(iosCapsLock);
    bind.sessionHandleFlutterKeyEvent(
        sessionId: sessionId,
        character: character,
        usbHid: usbHid,
        lockModes: lockModes,
        downOrUp: down);
  }

  void mapKeyboardModeRaw(RawKeyEvent e, bool iosCapsLock) {
    int positionCode = -1;
    int platformCode = -1;
    bool down;

    if (e.data is RawKeyEventDataMacOs) {
      RawKeyEventDataMacOs newData = e.data as RawKeyEventDataMacOs;
      positionCode = newData.keyCode;
      platformCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataWindows) {
      RawKeyEventDataWindows newData = e.data as RawKeyEventDataWindows;
      positionCode = newData.scanCode;
      platformCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataLinux) {
      RawKeyEventDataLinux newData = e.data as RawKeyEventDataLinux;
      // scanCode and keyCode of RawKeyEventDataLinux are incorrect.
      // 1. scanCode means keycode
      // 2. keyCode means keysym
      positionCode = newData.scanCode;
      platformCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataAndroid) {
      RawKeyEventDataAndroid newData = e.data as RawKeyEventDataAndroid;
      positionCode = newData.scanCode + 8;
      platformCode = newData.keyCode;
    } else {}

    if (e is RawKeyDownEvent) {
      down = true;
    } else {
      down = false;
    }
    inputRawKey(
        e.character ?? '', platformCode, positionCode, down, iosCapsLock);
  }

  /// Send raw Key Event
  void inputRawKey(String name, int platformCode, int positionCode, bool down,
      bool iosCapsLock) {
    final lockModes = _buildLockModes(iosCapsLock);
    bind.sessionHandleFlutterRawKeyEvent(
        sessionId: sessionId,
        name: name,
        platformCode: platformCode,
        positionCode: positionCode,
        lockModes: lockModes,
        downOrUp: down);
  }

  void legacyKeyboardModeRaw(RawKeyEvent e) {
    if (e is RawKeyDownEvent) {
      if (e.repeat) {
        sendRawKey(e, press: true);
      } else {
        sendRawKey(e, down: true);
      }
    }
    if (e is RawKeyUpEvent) {
      sendRawKey(e);
    }
  }

  void sendRawKey(RawKeyEvent e, {bool? down, bool? press}) {
    // for maximum compatibility
    final label = physicalKeyMap[e.physicalKey.usbHidUsage] ??
        logicalKeyMap[e.logicalKey.keyId] ??
        e.logicalKey.keyLabel;
    inputKey(label, down: down, press: press ?? false);
  }

  void legacyKeyboardMode(KeyEvent e) {
    if (e is KeyDownEvent) {
      sendKey(e, down: true);
    } else if (e is KeyRepeatEvent) {
      sendKey(e, press: true);
    } else if (e is KeyUpEvent) {
      sendKey(e);
    }
  }

  void sendKey(KeyEvent e, {bool? down, bool? press}) {
    // for maximum compatibility
    final label = physicalKeyMap[e.physicalKey.usbHidUsage] ??
        logicalKeyMap[e.logicalKey.keyId] ??
        e.logicalKey.keyLabel;
    inputKey(label, down: down, press: press ?? false);
  }

  /// Send key stroke event.
  /// [down] indicates the key's state(down or up).
  /// [press] indicates a click event(down and up).
  void inputKey(String name, {bool? down, bool? press}) {
    if (!keyboardPerm) return;
    if (isViewCamera) return;
    bind.sessionInputKey(
        sessionId: sessionId,
        name: name,
        down: down ?? false,
        press: press ?? true,
        alt: alt,
        ctrl: ctrl,
        shift: shift,
        command: command);
  }
}
