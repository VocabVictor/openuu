import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/brand_icon.dart';
import 'package:flutter_hbb/desktop/widgets/account_action.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

/// The main window's frameless title bar: brand on the left, a drag area
/// in the middle, the account entry and the window buttons on the right.
/// No tabs, no gear, no help icon live here; settings open from the sidebar.
class MainTitleBar extends StatelessWidget {
  static const double height = UiSpace.s10;
  static const Color background = Color(0xffeff4f7);

  const MainTitleBar({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
        height: height,
        color: background,
        child: Row(children: [
          const SizedBox(width: UiSpace.s3),
          const BrandIcon(size: 20),
          const SizedBox(width: UiSpace.s2),
          Text('OpenUU',
              style: UiType.of(context).rowTitle.copyWith(fontWeight: FontWeight.w500)),
          Expanded(
              child: DragToMoveArea(
                  child: GestureDetector(
                      behavior: HitTestBehavior.translucent,
                      onDoubleTap: () => toggleMaximize(true)
                          .then((maximized) =>
                              stateGlobal.setMaximized(maximized)),
                      child: const SizedBox.expand()))),
          const AccountAction(),
          if (!isMacOS && !kUseCompatibleUiMode) ...[
            ActionIcon(
                message: 'Minimize',
                icon: IconFont.min,
                iconSize: 16,
                boxSize: height,
                onTap: () => windowManager.minimize(),
                isClose: false),
            Obx(() => ActionIcon(
                message:
                    stateGlobal.isMaximized.isTrue ? 'Restore' : 'Maximize',
                icon: stateGlobal.isMaximized.isTrue
                    ? IconFont.restore
                    : IconFont.max,
                iconSize: 16,
                boxSize: height,
                onTap: () => toggleMaximize(true)
                    .then((maximized) => stateGlobal.setMaximized(maximized)),
                isClose: false)),
            ActionIcon(
                message: 'Close',
                icon: IconFont.close,
                iconSize: 16,
                boxSize: height,
                // The main window hides; the tray icon restores it.
                onTap: () => Future.delayed(
                    Duration.zero, () => windowManager.close()),
                isClose: true),
          ],
        ]));
  }
}
