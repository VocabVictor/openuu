import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../../../models/platform_model.dart';
import 'confirm_dialogs.dart';
import 'validation.dart';

void changeUnlockPinDialog(String oldPin, Function() callback) {
  final pinController = TextEditingController(text: oldPin);
  final confirmController = TextEditingController(text: oldPin);
  String? pinErrorText;
  String? confirmationErrorText;
  final maxLength = bind.mainMaxEncryptLen();
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      pinErrorText = null;
      confirmationErrorText = null;
      final pin = pinController.text.trim();
      final confirm = confirmController.text.trim();
      if (pin != confirm) {
        setState(() {
          confirmationErrorText =
              translate('The confirmation is not identical.');
        });
        return;
      }
      final errorMsg = bind.mainSetUnlockPin(pin: pin);
      if (errorMsg != '') {
        setState(() {
          pinErrorText = translate(errorMsg);
        });
        return;
      }
      callback.call();
      close();
    }

    return CustomAlertDialog(
      title: Text(translate("Set PIN")),
      content: Column(
        children: [
          DialogTextField(
            title: 'PIN',
            controller: pinController,
            obscureText: true,
            errorText: pinErrorText,
            maxLength: maxLength,
          ),
          DialogTextField(
            title: translate('Confirmation'),
            controller: confirmController,
            obscureText: true,
            errorText: confirmationErrorText,
            maxLength: maxLength,
          )
        ],
      ).marginOnly(bottom: 12),
      actions: [
        dialogButton(translate("Cancel"), onPressed: close, isOutline: true),
        dialogButton(translate("OK"), onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void checkUnlockPinDialog(String correctPin, Function() passCallback) {
  final controller = TextEditingController();
  String? errorText;
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      final pin = controller.text.trim();
      if (correctPin != pin) {
        setState(() {
          errorText = translate('Wrong PIN');
        });
        return;
      }
      passCallback.call();
      close();
    }

    return CustomAlertDialog(
      content: Row(
        children: [
          Expanded(
              child: PasswordWidget(
            title: 'PIN',
            controller: controller,
            errorText: errorText,
            hintText: '',
          ))
        ],
      ).marginOnly(bottom: 12),
      actions: [
        dialogButton(translate("Cancel"), onPressed: close, isOutline: true),
        dialogButton(translate("OK"), onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void confrimDeleteTrustedDevicesDialog(
    RxList<TrustedDevice> trustedDevices, RxList<Uint8List> selectedDevices) {
  CommonConfirmDialog(gFFI.dialogManager, '${translate('Confirm Delete')}?',
      () async {
    if (selectedDevices.isEmpty) return;
    if (selectedDevices.length == trustedDevices.length) {
      await bind.mainClearTrustedDevices();
      trustedDevices.clear();
      selectedDevices.clear();
    } else {
      final json = jsonEncode(selectedDevices.map((e) => e.toList()).toList());
      await bind.mainRemoveTrustedDevices(json: json);
      trustedDevices.removeWhere((element) {
        return selectedDevices.contains(element.hwid);
      });
      selectedDevices.clear();
    }
  });
}

void manageTrustedDeviceDialog() async {
  RxList<TrustedDevice> trustedDevices = (await TrustedDevice.get()).obs;
  RxList<Uint8List> selectedDevices = RxList.empty();
  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Manage trusted devices")),
      content: trustedDevicesTable(trustedDevices, selectedDevices),
      actions: [
        Obx(() => dialogButton(translate("Delete"),
                onPressed: selectedDevices.isEmpty
                    ? null
                    : () {
                        confrimDeleteTrustedDevicesDialog(
                          trustedDevices,
                          selectedDevices,
                        );
                      },
                isOutline: false)
            .marginOnly(top: 12)),
        dialogButton(translate("Close"), onPressed: close, isOutline: true)
            .marginOnly(top: 12),
      ],
      onCancel: close,
    );
  });
}

class TrustedDevice {
  late final Uint8List hwid;
  late final int time;
  late final String id;
  late final String name;
  late final String platform;

  TrustedDevice.fromJson(Map<String, dynamic> json) {
    final hwidList = json['hwid'] as List<dynamic>;
    hwid = Uint8List.fromList(hwidList.cast<int>());
    time = json['time'];
    id = json['id'];
    name = json['name'];
    platform = json['platform'];
  }

  String daysRemaining() {
    final expiry = time + 90 * 24 * 60 * 60 * 1000;
    final remaining = expiry - DateTime.now().millisecondsSinceEpoch;
    if (remaining < 0) {
      return '0';
    }
    return (remaining / (24 * 60 * 60 * 1000)).toStringAsFixed(0);
  }

  static Future<List<TrustedDevice>> get() async {
    final List<TrustedDevice> devices = List.empty(growable: true);
    try {
      final devicesJson = await bind.mainGetTrustedDevices();
      if (devicesJson.isNotEmpty) {
        final devicesList = json.decode(devicesJson);
        if (devicesList is List) {
          for (var device in devicesList) {
            devices.add(TrustedDevice.fromJson(device));
          }
        }
      }
    } catch (e) {
      print(e.toString());
    }
    devices.sort((a, b) => b.time.compareTo(a.time));
    return devices;
  }
}

Widget trustedDevicesTable(
    RxList<TrustedDevice> devices, RxList<Uint8List> selectedDevices) {
  RxBool selectAll = false.obs;
  setSelectAll() {
    if (selectedDevices.isNotEmpty &&
        selectedDevices.length == devices.length) {
      selectAll.value = true;
    } else {
      selectAll.value = false;
    }
  }

  devices.listen((_) {
    setSelectAll();
  });
  selectedDevices.listen((_) {
    setSelectAll();
  });
  return FittedBox(
    child: Obx(() => DataTable(
          columns: [
            DataColumn(
                label: Checkbox(
              value: selectAll.value,
              onChanged: (value) {
                if (value == true) {
                  selectedDevices.clear();
                  selectedDevices.addAll(devices.map((e) => e.hwid));
                } else {
                  selectedDevices.clear();
                }
              },
            )),
            DataColumn(label: Text(translate('Platform'))),
            DataColumn(label: Text(translate('ID'))),
            DataColumn(label: Text(translate('Username'))),
            DataColumn(label: Text(translate('Days remaining'))),
          ],
          rows: devices.map((device) {
            return DataRow(cells: [
              DataCell(Checkbox(
                value: selectedDevices.contains(device.hwid),
                onChanged: (value) {
                  if (value == null) return;
                  if (value) {
                    selectedDevices.remove(device.hwid);
                    selectedDevices.add(device.hwid);
                  } else {
                    selectedDevices.remove(device.hwid);
                  }
                },
              )),
              DataCell(Text(device.platform)),
              DataCell(Text(device.id)),
              DataCell(Text(device.name)),
              DataCell(Text(device.daysRemaining())),
            ]);
          }).toList(),
        )),
  );
}
