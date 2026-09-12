import 'dart:async';

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../address_book.dart';
import 'package:flutter_hbb/models/ab_model.dart';

void editAbTagDialog(
    List<dynamic> currentTags, Function(List<dynamic>) onSubmit) {
  var isInProgress = false;

  final tags = List.of(gFFI.abModel.currentAbTags);
  var selectedTag = currentTags.obs;

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      setState(() {
        isInProgress = true;
      });
      await onSubmit(selectedTag);
      close();
    }

    return CustomAlertDialog(
      title: Text(translate("Edit Tag")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: Wrap(
              children: tags
                  .map((e) => AddressBookTag(
                      name: e,
                      tags: selectedTag,
                      onTap: () {
                        if (selectedTag.contains(e)) {
                          selectedTag.remove(e);
                        } else {
                          selectedTag.add(e);
                        }
                      },
                      showActionMenu: false))
                  .toList(growable: false),
            ),
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void editAbPeerNoteDialog(String id) {
  var isInProgress = false;
  final currentNote = gFFI.abModel.getPeerNote(id);
  var controller = TextEditingController(text: currentNote);

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      setState(() {
        isInProgress = true;
      });
      await gFFI.abModel.changeNote(id: id, note: controller.text);
      close();
    }

    return CustomAlertDialog(
      title: Text(translate("Edit note")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          TextField(
            controller: controller,
            autofocus: true,
            maxLines: 3,
            minLines: 1,
            maxLength: 300,
            decoration: InputDecoration(
              labelText: translate('Note'),
            ),
          ).workaroundFreezeLinuxMint(),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void addPeersToAbDialog(
  List<Peer> peers,
) async {
  Future<bool> addTo(String abname) async {
    final mapList = peers.map((e) {
      var json = e.toJson();
      // remove password when add to another address book to avoid re-share
      json.remove('password');
      json.remove('hash');
      return json;
    }).toList();
    final errMsg = await gFFI.abModel.addPeersTo(mapList, abname);
    if (errMsg == null) {
      showToast(translate('Successful'));
      return true;
    } else {
      BotToast.showText(text: errMsg, contentColor: Colors.red);
      return false;
    }
  }

  // if only one address book and it is personal, add to it directly
  if (gFFI.abModel.addressbooks.length == 1 &&
      gFFI.abModel.current.isPersonal()) {
    await addTo(gFFI.abModel.currentName.value);
    return;
  }

  RxBool isInProgress = false.obs;
  final names = gFFI.abModel.addressBooksCanWrite();
  RxString currentName = gFFI.abModel.currentName.value.obs;
  TextEditingController controller = TextEditingController();
  if (gFFI.peerTabModel.currentTab == PeerTabIndex.ab.index) {
    names.remove(currentName.value);
  }
  if (names.isEmpty) {
    debugPrint('no address book to add peers to, should not happen');
    return;
  }
  if (!names.contains(currentName.value)) {
    currentName.value = names[0];
  }
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      if (controller.text != gFFI.abModel.translatedName(currentName.value)) {
        BotToast.showText(
            text: 'illegal address book name: ${controller.text}',
            contentColor: Colors.red);
        return;
      }
      isInProgress.value = true;
      if (await addTo(currentName.value)) {
        close();
      }
      isInProgress.value = false;
    }

    cancel() {
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(IconFont.addressBook, color: MyTheme.accent),
          Text(translate('Add to address book')).paddingOnly(left: 10),
        ],
      ),
      content: Obx(() => Column(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              // https://github.com/flutter/flutter/issues/145081
              DropdownMenu(
                initialSelection: currentName.value,
                onSelected: (value) {
                  if (value != null) {
                    currentName.value = value;
                  }
                },
                dropdownMenuEntries: names
                    .map((e) => DropdownMenuEntry(
                        value: e, label: gFFI.abModel.translatedName(e)))
                    .toList(),
                inputDecorationTheme: InputDecorationTheme(
                    isDense: true, border: UnderlineInputBorder()),
                enableFilter: true,
                controller: controller,
              ),
              // NOT use Offstage to wrap LinearProgressIndicator
              isInProgress.value ? const LinearProgressIndicator() : Offstage()
            ],
          )),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}
