import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';
import 'package:get/get.dart';

enum SessionPhase { connecting, waitingAccept, connected, disconnected }

/// State behind [SessionStatusBar] (docs/session-window-restyle.md §2 and
/// §4): the bar shows while connecting, waiting or disconnected, and hides
/// itself [UiSession.statusBarAutoHide] after the session is connected until
/// the pointer enters the top hot zone or the phase changes again.
class SessionStatusController {
  final phase = SessionPhase.connecting.obs;
  final direct = true.obs;
  final readOnly = false.obs;

  /// Seconds left before the automatic reconnect; disconnected phase only.
  final countdown = 0.obs;
  final visible = true.obs;
  Timer? _hide;
  Timer? _tick;

  void connecting() => _set(SessionPhase.connecting);

  void waitingAccept() => _set(SessionPhase.waitingAccept);

  void connected({required bool direct}) {
    this.direct.value = direct;
    _set(SessionPhase.connected);
    _armHide();
  }

  void disconnected({required int seconds, VoidCallback? onTimeout}) {
    countdown.value = seconds;
    _set(SessionPhase.disconnected);
    _tick?.cancel();
    _tick = Timer.periodic(const Duration(seconds: 1), (t) {
      if (countdown.value <= 1) {
        t.cancel();
        countdown.value = 0;
        onTimeout?.call();
      } else {
        countdown.value--;
      }
    });
  }

  /// Pointer entered the hot zone: show the bar, and hide it again later
  /// when the session is connected.
  void reveal() {
    visible.value = true;
    if (phase.value == SessionPhase.connected) _armHide();
  }

  void _set(SessionPhase p) {
    _hide?.cancel();
    _tick?.cancel();
    phase.value = p;
    visible.value = true;
  }

  void _armHide() {
    _hide?.cancel();
    _hide = Timer(UiSession.statusBarAutoHide, () => visible.value = false);
  }

  void dispose() {
    _hide?.cancel();
    _tick?.cancel();
  }
}

/// The status bars of the open remote sessions, by peer id, so the session
/// models can report a drop without reaching into the widget tree.
class SessionStatusRegistry {
  static final _bars = <String, SessionStatusController>{};

  static void register(String peerId, SessionStatusController controller) =>
      _bars[peerId] = controller;

  static void unregister(String peerId) => _bars.remove(peerId);

  static SessionStatusController? find(String peerId) => _bars[peerId];
}

/// The 28-high status strip at the top of a session canvas: a status dot,
/// a 12px caption, an optional read-only tag and, while disconnected, the
/// reconnect / disconnect buttons. Sits in a Stack; fills the width.
class SessionStatusBar extends StatelessWidget {
  const SessionStatusBar({
    Key? key,
    required this.controller,
    this.onReconnect,
    this.onDisconnect,
  }) : super(key: key);

  final SessionStatusController controller;
  final VoidCallback? onReconnect;
  final VoidCallback? onDisconnect;

  @override
  Widget build(BuildContext context) => Positioned(
      top: 0,
      left: 0,
      right: 0,
      child: Obx(() => AnimatedSwitcher(
          duration: UiSession.statusBarFade,
          child: controller.visible.isTrue
              ? _bar(context)
              : MouseRegion(
                  key: const ValueKey('hot-zone'),
                  onEnter: (_) => controller.reveal(),
                  child: const SizedBox(
                      height: UiSession.statusBarHotZone,
                      width: double.infinity),
                ))));

  Widget _bar(BuildContext context) => Obx(() {
        final phase = controller.phase.value;
        return KeyedSubtree(
          key: const ValueKey('bar'),
          child: Container(
            height: UiSession.statusBarHeight,
            padding: const EdgeInsets.symmetric(
                horizontal: UiSession.statusBarPaddingX),
            decoration: const BoxDecoration(
              color: Colors.white,
              border: Border(bottom: BorderSide(color: UiColor.border)),
            ),
            child: Row(children: [
              Container(
                width: UiSpace.statusDotSize,
                height: UiSpace.statusDotSize,
                decoration: BoxDecoration(
                    color: _dotColor(phase), shape: BoxShape.circle),
              ),
              const SizedBox(width: UiSpace.statusDotGap),
              Text(_caption(phase),
                  style: UiType.caption.copyWith(color: UiColor.textSecondary)),
              if (controller.readOnly.isTrue) ...[
                const SizedBox(width: UiSpace.s2),
                _tag(translate('Read-only')),
              ],
              const Spacer(),
              if (phase == SessionPhase.disconnected) ...[
                _button(translate('Reconnect now'), onReconnect),
                const SizedBox(width: UiSpace.s2),
                _button(translate('Disconnect'), onDisconnect, danger: true),
              ],
            ]),
          ),
        );
      });

  Color _dotColor(SessionPhase phase) {
    switch (phase) {
      case SessionPhase.connected:
        return UiColor.success;
      case SessionPhase.disconnected:
        return UiColor.danger;
      case SessionPhase.connecting:
      case SessionPhase.waitingAccept:
        return UiColor.warning;
    }
  }

  String _caption(SessionPhase phase) {
    switch (phase) {
      case SessionPhase.connecting:
        return translate('Connecting...');
      case SessionPhase.waitingAccept:
        return translate('Waiting for the peer to accept');
      case SessionPhase.connected:
        return translate(controller.direct.isTrue
            ? 'Connected via direct connection'
            : 'Connected via relay');
      case SessionPhase.disconnected:
        return translate('Disconnected, reconnecting in {} s')
            .replaceFirst('{}', controller.countdown.value.toString());
    }
  }

  Widget _tag(String text) => Container(
      height: UiSpace.tagHeight,
      padding: const EdgeInsets.symmetric(horizontal: UiSpace.tagPaddingX),
      alignment: Alignment.center,
      decoration: BoxDecoration(
          color: UiColor.primaryTint,
          borderRadius: BorderRadius.circular(UiSpace.tagRadius)),
      child: Text(text, style: UiType.tag));

  /// A 20-high inline button: secondary, or danger for disconnect.
  Widget _button(String label, VoidCallback? onPressed,
          {bool danger = false}) =>
      SizedBox(
          height: UiSpace.tagHeight,
          child: OutlinedButton(
              style: OutlinedButton.styleFrom(
                  foregroundColor: danger ? UiColor.danger : UiColor.text,
                  backgroundColor: Colors.white,
                  side: BorderSide(
                      color: danger ? UiColor.dangerBorder : UiColor.inputBorder),
                  padding: const EdgeInsets.symmetric(horizontal: UiSpace.s2),
                  minimumSize: Size.zero,
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(UiSpace.tagRadius)),
                  textStyle: UiType.caption.copyWith(fontWeight: FontWeight.w500)),
              onPressed: onPressed,
              child: Text(label)));
}
