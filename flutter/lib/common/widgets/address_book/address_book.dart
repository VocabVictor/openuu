import 'dart:math';

import 'package:bot_toast/bot_toast.dart';
import 'package:dropdown_button2/dropdown_button2.dart';
import 'package:dynamic_layouts/dynamic_layouts.dart';
import 'package:flutter/material.dart';
import 'package:openuu/common/formatter/id_formatter.dart';
import 'package:openuu/common/hbbs/hbbs.dart';
import 'package:openuu/common/widgets/peer_card.dart';
import 'package:openuu/common/widgets/peers_view.dart';
import 'package:openuu/consts.dart';
import 'package:openuu/desktop/widgets/popup_menu.dart';
import 'package:openuu/models/ab.dart';
import 'package:openuu/models/platform.dart';
import 'package:openuu/models/state.dart';
import 'package:url_launcher/url_launcher_string.dart';
import '../../../desktop/widgets/popup_menu_mod.dart' as mod_menu;
import 'package:get/get.dart';
import 'package:flex_color_picker/flex_color_picker.dart';

import '../../../common.dart';
import '../dialog.dart';
import '../login.dart';
part 'add_tag.dart';
part 'add_id.dart';
part 'tags.dart';
part 'dropdown.dart';
part 'layout.dart';
part 'tag_widget.dart';

final hideAbTagsPanel = false.obs;

class AddressBook extends StatefulWidget {
  final EdgeInsets? menuPadding;
  const AddressBook({Key? key, this.menuPadding}) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _AddressBookState();
  }
}

class _AddressBookState extends State<AddressBook> {
  var menuPos = RelativeRect.fill;

  @override
  Widget build(BuildContext context) => Obx(() {
        if (!gFFI.userModel.isLogin) {
          return Center(
              child: ElevatedButton(
                  onPressed: loginDialog, child: Text(translate("Login"))));
        } else if (gFFI.userModel.networkError.isNotEmpty) {
          return netWorkErrorWidget();
        } else {
          return Column(
            children: [
              // NOT use Offstage to wrap LinearProgressIndicator
              if (gFFI.abModel.currentAbLoading.value &&
                  gFFI.abModel.currentAbEmpty)
                const LinearProgressIndicator(),
              buildErrorBanner(context,
                  loading: gFFI.abModel.currentAbLoading,
                  err: gFFI.abModel.abPullError,
                  retry: null,
                  close: gFFI.abModel.clearPullErrors),
              buildErrorBanner(context,
                  loading: gFFI.abModel.currentAbLoading,
                  err: gFFI.abModel.currentAbPushError,
                  retry: null, // remove retry
                  close: () => gFFI.abModel.currentAbPushError.value = ''),
              Expanded(
                child: Obx(() => stateGlobal.isPortrait.isTrue
                    ? _buildAddressBookPortrait()
                    : _buildAddressBookLandscape()),
              ),
            ],
          );
        }
      });

}
