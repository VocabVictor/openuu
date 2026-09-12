part of 'input_model.dart';

class PointerEventToRust {
  final String kind;
  final String type;
  final dynamic value;

  PointerEventToRust(this.kind, this.type, this.value);

  Map<String, dynamic> toJson() {
    return {
      'k': kind,
      'v': {
        't': type,
        'v': value,
      }
    };
  }
}

class ToReleaseRawKeys {
  RawKeyEvent? lastLShiftKeyEvent;
  RawKeyEvent? lastRShiftKeyEvent;
  RawKeyEvent? lastLCtrlKeyEvent;
  RawKeyEvent? lastRCtrlKeyEvent;
  RawKeyEvent? lastLAltKeyEvent;
  RawKeyEvent? lastRAltKeyEvent;
  RawKeyEvent? lastLCommandKeyEvent;
  RawKeyEvent? lastRCommandKeyEvent;
  RawKeyEvent? lastSuperKeyEvent;

  reset() {
    lastLShiftKeyEvent = null;
    lastRShiftKeyEvent = null;
    lastLCtrlKeyEvent = null;
    lastRCtrlKeyEvent = null;
    lastLAltKeyEvent = null;
    lastRAltKeyEvent = null;
    lastLCommandKeyEvent = null;
    lastRCommandKeyEvent = null;
    lastSuperKeyEvent = null;
  }

  updateKeyDown(LogicalKeyboardKey logicKey, RawKeyDownEvent e) {
    if (e.isAltPressed) {
      if (logicKey == LogicalKeyboardKey.altLeft) {
        lastLAltKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.altRight) {
        lastRAltKeyEvent = e;
      }
    } else if (e.isControlPressed) {
      if (logicKey == LogicalKeyboardKey.controlLeft) {
        lastLCtrlKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.controlRight) {
        lastRCtrlKeyEvent = e;
      }
    } else if (e.isShiftPressed) {
      if (logicKey == LogicalKeyboardKey.shiftLeft) {
        lastLShiftKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.shiftRight) {
        lastRShiftKeyEvent = e;
      }
    } else if (e.isMetaPressed) {
      if (logicKey == LogicalKeyboardKey.metaLeft) {
        lastLCommandKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.metaRight) {
        lastRCommandKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.superKey) {
        lastSuperKeyEvent = e;
      }
    }
  }

  updateKeyUp(LogicalKeyboardKey logicKey, RawKeyUpEvent e) {
    if (e.isAltPressed) {
      if (logicKey == LogicalKeyboardKey.altLeft) {
        lastLAltKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.altRight) {
        lastRAltKeyEvent = null;
      }
    } else if (e.isControlPressed) {
      if (logicKey == LogicalKeyboardKey.controlLeft) {
        lastLCtrlKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.controlRight) {
        lastRCtrlKeyEvent = null;
      }
    } else if (e.isShiftPressed) {
      if (logicKey == LogicalKeyboardKey.shiftLeft) {
        lastLShiftKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.shiftRight) {
        lastRShiftKeyEvent = null;
      }
    } else if (e.isMetaPressed) {
      if (logicKey == LogicalKeyboardKey.metaLeft) {
        lastLCommandKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.metaRight) {
        lastRCommandKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.superKey) {
        lastSuperKeyEvent = null;
      }
    }
  }

  release(KeyEventResult Function(RawKeyEvent e) handleRawKeyEvent) {
    for (final key in [
      lastLShiftKeyEvent,
      lastRShiftKeyEvent,
      lastLCtrlKeyEvent,
      lastRCtrlKeyEvent,
      lastLAltKeyEvent,
      lastRAltKeyEvent,
      lastLCommandKeyEvent,
      lastRCommandKeyEvent,
      lastSuperKeyEvent,
    ]) {
      if (key != null) {
        handleRawKeyEvent(RawKeyUpEvent(
          data: key.data,
          character: key.character,
        ));
      }
    }
  }
}

class ToReleaseKeys {
  KeyEvent? lastLShiftKeyEvent;
  KeyEvent? lastRShiftKeyEvent;
  KeyEvent? lastLCtrlKeyEvent;
  KeyEvent? lastRCtrlKeyEvent;
  KeyEvent? lastLAltKeyEvent;
  KeyEvent? lastRAltKeyEvent;
  KeyEvent? lastLCommandKeyEvent;
  KeyEvent? lastRCommandKeyEvent;
  KeyEvent? lastSuperKeyEvent;

  reset() {
    lastLShiftKeyEvent = null;
    lastRShiftKeyEvent = null;
    lastLCtrlKeyEvent = null;
    lastRCtrlKeyEvent = null;
    lastLAltKeyEvent = null;
    lastRAltKeyEvent = null;
    lastLCommandKeyEvent = null;
    lastRCommandKeyEvent = null;
    lastSuperKeyEvent = null;
  }

  release(KeyEventResult Function(KeyEvent e) handleKeyEvent) {
    for (final key in [
      lastLShiftKeyEvent,
      lastRShiftKeyEvent,
      lastLCtrlKeyEvent,
      lastRCtrlKeyEvent,
      lastLAltKeyEvent,
      lastRAltKeyEvent,
      lastLCommandKeyEvent,
      lastRCommandKeyEvent,
      lastSuperKeyEvent,
    ]) {
      if (key != null) {
        handleKeyEvent(key);
      }
    }
  }
}
