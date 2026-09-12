part of 'relative_mouse_model.dart';

extension RelativeMouseNative on RelativeMouseModel {
  // TODO(perf): Consider routing native delta through RelativeMouseAccumulator/throttle
  // if high-polling mice (e.g. 1000Hz+) cause message flooding on the network.
  void _onNativeMouseDelta(int dx, int dy) {
    if (!enabled.value) return;
    // Send directly to remote without accumulator (native already provides integer deltas)
    _sendMouseMessageToSession({
      'type': 'move_relative',
      'x': '$dx',
      'y': '$dy',
    });
  }

  Future<bool> _enableNativeRelativeMouseMode() async {
    if (!isMacOS) return false;
    if (RelativeMouseModel._hostChannel == null) {
      RelativeMouseModel.initHostChannel();
      if (RelativeMouseModel._hostChannel == null) return false;
    }

    // Defensive guard: prevent overwriting an already-active native session.
    // In practice, this should not happen because when relative mouse mode is active,
    // the cursor is locked and the user cannot switch to another session window.
    // The user must first exit relative mouse mode (via Cmd+G on macOS or Ctrl+Alt on
    // Windows/Linux) before interacting with a different session.
    if (RelativeMouseModel._activeNativeModel != null && RelativeMouseModel._activeNativeModel != this) {
      debugPrint(
          '[RelMouse] Another model already has native relative mouse mode active');
      return false;
    }

    try {
      final result =
          await RelativeMouseModel._hostChannel!.invokeMethod('enableNativeRelativeMouseMode');
      if (result == true) {
        RelativeMouseModel._activeNativeModel = this;
        return true;
      }
    } catch (e) {
      debugPrint('[RelMouse] Failed to enable native relative mouse mode: $e');
    }
    return false;
  }

  Future<void> _disableNativeRelativeMouseMode() async {
    if (!isMacOS) return;
    if (RelativeMouseModel._hostChannel == null) return;

    // Only the owning model should disable native mode to avoid
    // one session inadvertently disrupting another's native relative mouse state.
    if (RelativeMouseModel._activeNativeModel != this) {
      return;
    }

    try {
      await RelativeMouseModel._hostChannel!.invokeMethod('disableNativeRelativeMouseMode');
    } catch (e) {
      debugPrint('[RelMouse] Failed to disable native relative mouse mode: $e');
    } finally {
      if (RelativeMouseModel._activeNativeModel == this) {
        RelativeMouseModel._activeNativeModel = null;
      }
    }
  }
}
