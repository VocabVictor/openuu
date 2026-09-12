import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../../../models/platform_model.dart';
import 'validation.dart';

void changeIdDialog() {
  var newId = "";
  var msg = "";
  var isInProgress = false;
  TextEditingController controller = TextEditingController();
  final RxString rxId = controller.text.trim().obs;

  final rules = [
    RegexValidationRule('starts with a letter', RegExp(r'^[a-zA-Z]')),
    LengthRangeValidationRule(6, 16),
    RegexValidationRule('allowed characters', RegExp(r'^[\w-]*$'))
  ];

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      debugPrint("onSubmit");
      newId = controller.text.trim();

      final Iterable violations = rules.where((r) => !r.validate(newId));
      if (violations.isNotEmpty) {
        setState(() {
          msg = (isDesktop || isWebDesktop)
              ? '${translate('Prompt')}:  ${violations.map((r) => r.name).join(', ')}'
              : violations.map((r) => r.name).join(', ');
        });
        return;
      }

      setState(() {
        msg = "";
        isInProgress = true;
        bind.mainChangeId(newId: newId);
      });

      var status = await bind.mainGetAsyncStatus();
      while (status == " ") {
        await Future.delayed(const Duration(milliseconds: 100));
        status = await bind.mainGetAsyncStatus();
      }
      if (status.isEmpty) {
        // ok
        close();
        return;
      }
      setState(() {
        isInProgress = false;
        msg = (isDesktop || isWebDesktop)
            ? '${translate('Prompt')}: ${translate(status)}'
            : translate(status);
      });
    }

    return CustomAlertDialog(
      title: Text(translate("Change ID")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("id_change_tip")),
          const SizedBox(
            height: 12.0,
          ),
          TextField(
            decoration: InputDecoration(
                labelText: translate('Your new ID'),
                errorText: msg.isEmpty ? null : translate(msg),
                suffixText: '${rxId.value.length}/16',
                suffixStyle: const TextStyle(fontSize: 12, color: Colors.grey)),
            inputFormatters: [
              LengthLimitingTextInputFormatter(16),
              // FilteringTextInputFormatter(RegExp(r"[a-zA-z][a-zA-z0-9\_]*"), allow: true)
            ],
            controller: controller,
            autofocus: true,
            onChanged: (value) {
              setState(() {
                rxId.value = value.trim();
                msg = '';
              });
            },
          ).workaroundFreezeLinuxMint(),
          const SizedBox(
            height: 8.0,
          ),
          (isDesktop || isWebDesktop)
              ? Obx(() => Wrap(
                    runSpacing: 8,
                    spacing: 4,
                    children: rules.map((e) {
                      var checked = e.validate(rxId.value);
                      return Chip(
                          label: Text(
                            e.name,
                            style: TextStyle(
                                color: checked
                                    ? const Color(0xFF0A9471)
                                    : Color.fromARGB(255, 198, 86, 157)),
                          ),
                          backgroundColor: checked
                              ? const Color(0xFFD0F7ED)
                              : Color.fromARGB(255, 247, 205, 232));
                    }).toList(),
                  )).marginOnly(bottom: 8)
              : SizedBox.shrink(),
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

void changeIdWhiteList({Function()? callback}) async {
  final curIdWhiteList = await bind.mainGetOption(key: kOptionIdWhitelist);
  var newIdWhiteListField = curIdWhiteList == defaultOptionWhitelist
      ? ''
      : curIdWhiteList.split(',').join('\n');
  var controller = TextEditingController(text: newIdWhiteListField);
  var msg = "";
  var isInProgress = false;
  final isOptFixed = isOptionFixed(kOptionIdWhitelist);
  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("ID whitelisting")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("whitelist_sep")),
          const SizedBox(
            height: 8.0,
          ),
          Text(translate("id_whitelist_wildcard_tip")),
          const SizedBox(
            height: 8.0,
          ),
          Text(translate("id_whitelist_caveat_tip")),
          const SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        decoration: InputDecoration(
                          errorText: msg.isEmpty ? null : translate(msg),
                        ),
                        controller: controller,
                        enabled: !isOptFixed,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
          const SizedBox(
            height: 4.0,
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        if (!isOptFixed)
          dialogButton("Clear", onPressed: () async {
            await bind.mainSetOption(
                key: kOptionIdWhitelist, value: defaultOptionWhitelist);
            callback?.call();
            close();
          }, isOutline: true),
        if (!isOptFixed)
          dialogButton(
            "OK",
            onPressed: () async {
              setState(() {
                msg = "";
                isInProgress = true;
              });
              newIdWhiteListField = controller.text.trim();
              var newIdWhiteList = "";
              if (newIdWhiteListField.isEmpty) {
                // pass
              } else {
                final ids = newIdWhiteListField
                    .trim()
                    .split(RegExp(r"[\s,;\n]+"))
                    .where((e) => e.isNotEmpty)
                    .toList();
                // Separators are handled above; allow all other Unicode characters.
                for (final id in ids) {
                  final hasControlCharacters = id.runes.any(
                      (char) => char <= 0x1f || (char >= 0x7f && char <= 0x9f));
                  if (hasControlCharacters) {
                    msg = "${translate("Invalid ID")} $id";
                    setState(() {
                      isInProgress = false;
                    });
                    return;
                  }
                }
                newIdWhiteList = ids.join(',');
              }
              if (newIdWhiteList.trim().isEmpty) {
                newIdWhiteList = defaultOptionWhitelist;
              }
              await bind.mainSetOption(
                  key: kOptionIdWhitelist, value: newIdWhiteList);
              callback?.call();
              close();
            },
          ),
      ],
      onCancel: close,
    );
  });
}
