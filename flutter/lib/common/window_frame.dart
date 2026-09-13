
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

import '../consts.dart';

import 'globals.dart';
import 'windows_misc.dart';

// Only used on Linux.
// `windowManager.setResizable(false)` will reset the window size to the default size on Linux.
// https://stackoverflow.com/questions/8193613/gtk-window-resize-disable-without-going-back-to-default
// So we need to use this flag to enable/disable resizable.
bool _linuxWindowResizable = true;

// https://github.com/leanflutter/window_manager/blob/87dd7a50b4cb47a375b9fc697f05e56eea0a2ab3/lib/src/widgets/virtual_window_frame.dart#L44
Widget buildVirtualWindowFrame(BuildContext context, Widget child) {
  boxShadow() => isMainDesktopWindow
      ? <BoxShadow>[
          if (stateGlobal.fullscreen.isFalse || stateGlobal.isMaximized.isFalse)
            BoxShadow(
              color: Colors.black.withOpacity(0.1),
              offset: Offset(
                  0.0,
                  stateGlobal.isFocused.isTrue
                      ? kFrameBoxShadowOffsetFocused
                      : kFrameBoxShadowOffsetUnfocused),
              blurRadius: kFrameBoxShadowBlurRadius,
            ),
        ]
      : null;
  return Obx(
    () => Container(
      decoration: BoxDecoration(
        color: isMainDesktopWindow
            ? Colors.transparent
            : Theme.of(context).colorScheme.background,
        border: Border.all(
          color: Theme.of(context).dividerColor,
          width: stateGlobal.windowBorderWidth.value,
        ),
        borderRadius: BorderRadius.circular(
          (stateGlobal.fullscreen.isTrue || stateGlobal.isMaximized.isTrue)
              ? 0
              : kFrameBorderRadius,
        ),
        boxShadow: boxShadow(),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(
          (stateGlobal.fullscreen.isTrue || stateGlobal.isMaximized.isTrue)
              ? 0
              : kFrameClipRRectBorderRadius,
        ),
        child: child,
      ),
    ),
  );
}

get windowResizeEdgeSize =>
    isLinux && !_linuxWindowResizable ? 0.0 : kWindowResizeEdgeSize;

// `windowManager.setResizable(false)` will reset the window size to the default size on Linux and then set unresizable.
// See _linuxWindowResizable for more details.
// So we use `setResizable()` instead of `windowManager.setResizable()`.
//
// We can only call `windowManager.setResizable(false)` if we need the default size on Linux.
setResizable(bool resizable) {
  if (isLinux) {
    _linuxWindowResizable = resizable;
    stateGlobal.refreshResizeEdgeSize();
  } else {
    windowManager.setResizable(resizable);
  }
}

// `setMovable()` is only supported on macOS.
//
// On macOS, the window can be dragged by the tab bar by default.
// We need to disable the movable feature to prevent the window from being dragged by the tabs in the tab bar.
//
// When we drag the blank tab bar (not the tab), the window will be dragged normally by adding the `onPanStart` handle.
//
// See the following code for more details:
// https://github.com/rustdesk/rustdesk/blob/ce1dac3b8613596b4d8ae981275f9335489eb935/flutter/lib/desktop/widgets/tabbar_widget.dart#L385
// https://github.com/rustdesk/rustdesk/blob/ce1dac3b8613596b4d8ae981275f9335489eb935/flutter/lib/desktop/widgets/tabbar_widget.dart#L399
//
// @platforms macos
disableWindowMovable(int? windowId) {
  if (!isMacOS) {
    return;
  }

  if (windowId == null) {
    windowManager.setMovable(false);
  } else {
    WindowController.fromWindowId(windowId).setMovable(false);
  }
}

List<ResizeEdge>? get windowManagerEnableResizeEdges => isWindows
    ? [
        ResizeEdge.topLeft,
        ResizeEdge.top,
        ResizeEdge.topRight,
      ]
    : null;

List<SubWindowResizeEdge>? get subWindowManagerEnableResizeEdges => isWindows
    ? [
        SubWindowResizeEdge.topLeft,
        SubWindowResizeEdge.top,
        SubWindowResizeEdge.topRight,
      ]
    : null;

void earlyAssert() {
  assert('\1' == '1');
}

// https://github.com/flutter/flutter/issues/153560#issuecomment-2497160535
// For TextField, TextFormField
extension WorkaroundFreezeLinuxMint on Widget {
  Widget workaroundFreezeLinuxMint() {
    // No need to check if is Linux Mint, because this workaround is harmless on other platforms.
    if (isLinux) {
      return ExcludeSemantics(child: this);
    } else {
      return this;
    }
  }
}

// Don't use `extension` here, the border looks weird if using `extension` in my test.
Widget workaroundWindowBorder(BuildContext context, Widget child) {
  if (!isWin10) {
    return child;
  }

  final isLight = Theme.of(context).brightness == Brightness.light;
  final borderColor = isLight ? Colors.black87 : Colors.grey;
  final width = isLight ? 0.5 : 0.1;

  getBorderWidget(Widget child) {
    return Obx(() =>
        (stateGlobal.isMaximized.isTrue || stateGlobal.fullscreen.isTrue)
            ? Offstage()
            : child);
  }

  final List<Widget> borders = [
    getBorderWidget(Container(
      color: borderColor,
      height: width + 0.1,
    ))
  ];
  if (kWindowType == WindowType.Main && !isLight) {
    borders.addAll([
      getBorderWidget(Align(
        alignment: Alignment.topLeft,
        child: Container(
          color: borderColor,
          width: width,
        ),
      )),
      getBorderWidget(Align(
        alignment: Alignment.topRight,
        child: Container(
          color: borderColor,
          width: width,
        ),
      )),
      getBorderWidget(Align(
        alignment: Alignment.bottomCenter,
        child: Container(
          color: borderColor,
          height: width,
        ),
      )),
    ]);
  }
  return Stack(
    children: [
      child,
      ...borders,
    ],
  );
}
