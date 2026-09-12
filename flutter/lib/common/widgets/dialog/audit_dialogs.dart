import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:get/get.dart';
import 'package:flutter_hbb/utils/http_service.dart' as http;

import '../../../common.dart';
import '../../../models/model.dart';
import '../../../models/platform_model.dart';
import 'session_dialogs.dart';

showAuditDialog(FFI ffi) async {
  final controller = TextEditingController(
      text: bind.sessionGetLastAuditNote(sessionId: ffi.sessionId));
  ffi.dialogManager.show((setState, close, context) {
    submit() {
      var text = controller.text;
      bind.sessionSendNote(sessionId: ffi.sessionId, note: text);
      close();
    }

    return CustomAlertDialog(
      title: Text(translate('Note')),
      content: SizedBox(
          width: 250,
          height: 120,
          child: buildNoteTextField(
            controller: controller,
            onEscape: close,
          )),
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit)
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

bool allowAskForNoteAtEndOfConnection(FFI? ffi, bool closedByControlling) {
  if (ffi == null) {
    return false;
  }
  return mainGetLocalBoolOptionSync(kOptionAllowAskForNoteAtEndOfConnection) &&
      bind
          .sessionGetAuditServerSync(sessionId: ffi.sessionId, typ: "conn")
          .isNotEmpty &&
      bind.sessionGetAuditGuid(sessionId: ffi.sessionId).isNotEmpty &&
      bind.sessionGetLastAuditNote(sessionId: ffi.sessionId).isEmpty &&
      (!closedByControlling ||
          bind.willSessionCloseCloseSession(sessionId: ffi.sessionId));
}

// return value: close canceled
//  true: return
//  false: go on
Future<bool> desktopTryShowTabAuditDialogCloseCancelled(
    {required String id, required DesktopTabController tabController}) async {
  try {
    final page =
        tabController.state.value.tabs.firstWhere((tab) => tab.key == id).page;
    final ffi = (page as dynamic).ffi;
    final res = await showConnEndAuditDialogCloseCanceled(ffi: ffi);
    return res;
  } catch (e) {
    debugPrint('Failed to show audit dialog: $e');
    return false;
  }
}

// return value:
//  true: return
//  false: go on
Future<bool> showConnEndAuditDialogCloseCanceled(
    {required FFI ffi, String? type, String? title, String? text}) async {
  final res = await _showConnEndAuditDialogCloseCanceled(
      ffi: ffi, type: type, title: title, text: text);
  if (res == true) {
    return true;
  }
  return false;
}

// return value:
//  true: return
//  false / null: go on
Future<bool?> _showConnEndAuditDialogCloseCanceled({
  required FFI ffi,
  String? type,
  String? title,
  String? text,
}) async {
  final closedByControlling = type == null;
  final showDialog = allowAskForNoteAtEndOfConnection(ffi, closedByControlling);
  if (!showDialog) {
    return false;
  }
  ffi.dialogManager.dismissAll();

  Future<void> updateAuditNoteByGuid(String auditGuid, String note) async {
    debugPrint('Updating audit note for GUID: $auditGuid, note: $note');
    try {
      final apiServer = await bind.mainGetApiServer();
      if (apiServer.isEmpty) {
        debugPrint('API server is empty, cannot update audit note');
        return;
      }
      final url = '$apiServer/api/audit';
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        'guid': auditGuid,
        'note': note,
      });

      final response = await http.put(
        Uri.parse(url),
        headers: headers,
        body: body,
      );

      if (response.statusCode == 200) {
        debugPrint('Successfully updated audit note for GUID: $auditGuid');
      } else {
        debugPrint(
            'Failed to update audit note. Status: ${response.statusCode}, Body: ${response.body}');
      }
    } catch (e) {
      debugPrint('Error updating audit note: $e');
    }
  }

  final controller = TextEditingController();
  bool askForNote =
      mainGetLocalBoolOptionSync(kOptionAllowAskForNoteAtEndOfConnection);
  final isOptFixed = isOptionFixed(kOptionAllowAskForNoteAtEndOfConnection);
  bool isInProgress = false;

  return await ffi.dialogManager.show<bool>((setState, close, context) {
    cancel() {
      close(true);
    }

    set() async {
      if (isInProgress) return;
      setState(() {
        isInProgress = true;
      });
      var text = controller.text;
      if (text.isNotEmpty) {
        await updateAuditNoteByGuid(
                bind.sessionGetAuditGuid(sessionId: ffi.sessionId), text)
            .timeout(const Duration(seconds: 6), onTimeout: () {
          debugPrint('updateAuditNoteByGuid timeout after 6s');
        });
      }
      // Save the "ask for note" preference
      if (!isOptFixed) {
        await mainSetLocalBoolOption(
            kOptionAllowAskForNoteAtEndOfConnection, askForNote);
      }
    }

    submit() async {
      await set();
      close(false);
    }

    final buttons = [
      dialogButton('OK', onPressed: isInProgress ? null : submit)
    ];
    if (type == 'relay-hint' || type == 'relay-hint2') {
      buttons.add(dialogButton('Retry', onPressed: () async {
        await set();
        close(true);
        ffi.ffiModel.reconnect(ffi.dialogManager, ffi.sessionId, false);
      }));
      if (type == 'relay-hint2') {
        buttons.add(dialogButton('Connect via relay', onPressed: () async {
          await set();
          close(true);
          ffi.ffiModel.reconnect(ffi.dialogManager, ffi.sessionId, true);
        }));
      }
    }
    if (closedByControlling) {
      buttons.add(dialogButton('Cancel',
          onPressed: isInProgress ? null : cancel, isOutline: true));
    }

    Widget content;
    if (closedByControlling) {
      content = SelectionArea(
          child: msgboxContent(
              'info', 'Close', 'Are you sure to close the connection?'));
    } else {
      content =
          SelectionArea(child: msgboxContent(type, title ?? '', text ?? ''));
    }

    return CustomAlertDialog(
      title: null,
      content: SizedBox(
          width: 350,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              content,
              const SizedBox(height: 16),
              SizedBox(
                height: 120,
                child: buildNoteTextField(
                  controller: controller,
                  onEscape: cancel,
                ),
              ),
              if (!isOptFixed) ...[
                const SizedBox(height: 8),
                InkWell(
                  onTap: () {
                    setState(() {
                      askForNote = !askForNote;
                    });
                  },
                  child: Row(
                    children: [
                      Checkbox(
                        value: askForNote,
                        onChanged: (value) {
                          setState(() {
                            askForNote = value ?? false;
                          });
                        },
                      ),
                      Expanded(
                        child: Text(
                          translate('note-at-conn-end-tip'),
                          style: const TextStyle(fontSize: 13),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              if (isInProgress)
                const LinearProgressIndicator().marginOnly(top: 4),
            ],
          )),
      actions: buttons,
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}
