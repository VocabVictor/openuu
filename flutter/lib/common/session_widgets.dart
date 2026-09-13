import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../consts.dart';
import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'my_theme.dart';

Future<bool> canBeBlocked() async {
  // First check control permission
  final controlPermission = await bind.mainGetCommon(
      key: "is-remote-modify-enabled-by-control-permissions");
  if (controlPermission == "true") {
    return false;
  } else if (controlPermission == "false") {
    return true;
  }

  // Check local settings
  var accessMode = await bind.mainGetOption(key: kOptionAccessMode);
  var isCustomAccessMode = accessMode != 'full' && accessMode != 'view';
  var option = option2bool(kOptionAllowRemoteConfigModification,
      await bind.mainGetOption(key: kOptionAllowRemoteConfigModification));
  return accessMode == 'view' || (isCustomAccessMode && !option);
}

// to-do: web not implemented
Future<void> shouldBeBlocked(RxBool block, WhetherUseRemoteBlock? use) async {
  if (use != null && !await use()) {
    block.value = false;
    return;
  }
  var time0 = DateTime.now().millisecondsSinceEpoch;
  await bind.mainCheckMouseTime();
  Timer(const Duration(milliseconds: 120), () async {
    var d = time0 - await bind.mainGetMouseTime();
    if (d < 120) {
      block.value = true;
    } else {
      block.value = false;
    }
  });
}

typedef WhetherUseRemoteBlock = Future<bool> Function();

Widget buildRemoteBlock(
    {required Widget child,
    required RxBool block,
    required bool mask,
    WhetherUseRemoteBlock? use}) {
  return Obx(() => MouseRegion(
        onEnter: (_) async {
          await shouldBeBlocked(block, use);
        },
        onExit: (event) => block.value = false,
        child: Stack(children: [
          // scope block tab
          preventMouseKeyBuilder(child: child, block: block.value),
          // mask block click, cm not block click and still use check_click_time to avoid block local click
          if (mask)
            Offstage(
                offstage: !block.value,
                child: Container(
                  color: Colors.black.withOpacity(0.5),
                )),
        ]),
      ));
}

Widget preventMouseKeyBuilder({required Widget child, required bool block}) {
  return ExcludeFocus(
      excluding: block, child: AbsorbPointer(child: child, absorbing: block));
}

Widget unreadMessageCountBuilder(RxInt? count,
    {double? size, double? fontSize}) {
  return Obx(() => Offstage(
      offstage: !((count?.value ?? 0) > 0),
      child: Container(
        width: size ?? 16,
        height: size ?? 16,
        decoration: BoxDecoration(
          color: Colors.red,
          shape: BoxShape.circle,
        ),
        child: Center(
          child: Text("${count?.value ?? 0}",
              maxLines: 1,
              style: TextStyle(color: Colors.white, fontSize: fontSize ?? 10)),
        ),
      )));
}

Widget unreadTopRightBuilder(RxInt? count, {Widget? icon}) {
  return Stack(
    children: [
      icon ?? Icon(Icons.chat),
      Positioned(
          top: 0,
          right: 0,
          child: unreadMessageCountBuilder(count, size: 12, fontSize: 8))
    ],
  );
}

Widget buildErrorBanner(BuildContext context,
    {required RxBool loading,
    required RxString err,
    required Function? retry,
    required Function close}) {
  return Obx(() => Offstage(
        offstage: !(!loading.value && err.value.isNotEmpty),
        child: Center(
            child: Container(
          color: MyTheme.color(context).errorBannerBg,
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              FittedBox(
                child: Icon(
                  Icons.info,
                  color: Color.fromARGB(255, 249, 81, 81),
                ),
              ).marginAll(4),
              Flexible(
                child: Align(
                    alignment: Alignment.centerLeft,
                    child: Tooltip(
                      message: translate(err.value),
                      child: SelectableText(
                        translate(err.value),
                      ),
                    )).marginSymmetric(vertical: 2),
              ),
              if (retry != null)
                InkWell(
                    onTap: () {
                      retry.call();
                    },
                    child: Text(
                      translate("Retry"),
                      style: TextStyle(color: MyTheme.accent),
                    )).marginSymmetric(horizontal: 5),
              FittedBox(
                child: InkWell(
                  onTap: () {
                    close.call();
                  },
                  child: Icon(Icons.close).marginSymmetric(horizontal: 5),
                ),
              ).marginAll(4)
            ],
          ),
        )).marginOnly(bottom: 14),
      ));
}
