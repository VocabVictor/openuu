part of 'input_model.dart';

/// Side mouse buttons (back / forward) on Linux.
///
/// Flutter's Linux embedder drops X11 button 8/9 events, so they are captured
/// natively through GDK and forwarded over a platform channel. That channel is
/// per engine, not per session, so the routing is static: which session the
/// pointer is over, and which session received the press of each button.
class SideButtons {
  static InputModel? _active;
  // Which session received the down event of each button, so the matching up
  // is routed there even if the pointer has left the view or another button
  // was pressed in between.
  static final Map<MouseButtons, InputModel> _downModels = {};
  static bool _channelInitialized = false;

  /// Each Flutter engine (main window + sub-windows from desktop_multi_window)
  /// runs its own Dart isolate with its own statics. Called from initEnv()
  /// which runs per-engine, so each isolate registers its own handler tied
  /// to its own set of InputModels.
  static void initChannel() {
    if (!isLinux) return;
    if (_channelInitialized) return;
    _channelInitialized = true;

    const channel = MethodChannel('org.rustdesk.rustdesk/side_buttons');
    channel.setMethodCallHandler((call) async {
      if (call.method == 'onSideMouseButton') {
        final args = call.arguments as Map<dynamic, dynamic>;
        final button = args['button'] as String;
        final type = args['type'] as String;
        final mb = button == 'back' ? MouseButtons.back : MouseButtons.forward;

        if (type == 'down') {
          final model = _active;
          if (model != null &&
              !(model.isViewOnly && !model.showMyCursor) &&
              model.keyboardPerm &&
              !model.isViewCamera) {
            _downModels[mb] = model;
            _send(model, type, mb);
          }
        } else {
          // Only route 'up' when we recorded the matching 'down';
          // dropping avoids sending unpaired 'up' to an unrelated session.
          final model = _downModels.remove(mb);
          if (model != null) {
            _send(model, type, mb);
          }
        }
      }
      return null;
    });
  }

  /// Bypasses the permission check so a release always goes through even if
  /// permissions changed after the press, and does not block the platform
  /// channel handler.
  static void _send(InputModel model, String type, MouseButtons mb) {
    unawaited(model._sendMouseUnchecked(type, mb).catchError((Object e) {
      debugPrint('[InputModel] failed to send side button $type for $mb: $e');
    }));
  }

  /// The session the pointer is over is the one a side button belongs to.
  static void onEnterOrLeave(InputModel model, bool enter) {
    if (enter) {
      _active = model;
    } else if (_active == model) {
      _active = null;
    }
  }

  /// Releases the buttons [model] still holds on the peer, so closing a
  /// session mid-press does not leave a stuck button, and drops the stale
  /// routing entries.
  static void forget(InputModel model) {
    if (_active == model) _active = null;
    final held = _downModels.entries
        .where((e) => e.value == model)
        .map((e) => e.key)
        .toList();
    for (final mb in held) {
      _downModels.remove(mb);
      // Best-effort release; session may already be tearing down.
      _send(model, 'up', mb);
    }
  }
}
