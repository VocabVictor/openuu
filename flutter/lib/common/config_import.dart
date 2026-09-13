import 'dart:convert';

import 'package:flutter/material.dart';

import '../common.dart';
import '../models/platform_model.dart';

/// Previews a provisioning text (`openuu://config/…`, the legacy payload or
/// a JSON file) in a confirmation dialog and applies it only on confirm
/// (docs/server-config-provisioning.md §4). `trusted` is true only for a
/// file the user picked. Returns true when the import was applied.
Future<bool> confirmImportConfig(String text, {required bool trusted}) async {
  final preview = _json(await bind.mainPreviewConfigText(text: text));
  if (preview['ok'] != true) {
    showToast(
        preview['error']?.toString() ?? translate('Invalid server configuration'));
    return false;
  }
  final server = (preview['server'] as Map?) ?? {};
  final locked = (preview['locked'] as List?) ?? [];
  var applied = false;
  await gFFI.dialogManager.show((setState, close, context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    Widget line(String label, String value) => Padding(
        padding: const EdgeInsets.only(bottom: 4),
        child: Row(children: [
          SizedBox(width: 96, child: Text(label)),
          Expanded(child: Text(value.isEmpty ? '—' : value)),
        ]));
    return CustomAlertDialog(
        title: Text(translate('Import server config')),
        content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              line(translate('ID Server'), '${server['id'] ?? ''}'),
              line(translate('Relay Server'), '${server['relay'] ?? ''}'),
              line(translate('API Server'), '${server['api'] ?? ''}'),
              line('Key',
                  (server['key'] ?? '').toString().isEmpty ? '' : '••••••'),
              if (locked.isNotEmpty)
                Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(
                        '${zh ? '将锁定' : 'Locked'}: ${locked.join(', ')}',
                        style: const TextStyle(fontSize: 12))),
            ]),
        actions: [
          dialogButton('Cancel', onPressed: close, isOutline: true),
          dialogButton('OK', onPressed: () async {
            final report = _json(
                await bind.mainImportConfigText(text: text, trusted: trusted));
            close();
            if (report['ok'] == true) {
              applied = true;
              showToast(translate('Import server configuration successfully'));
            } else {
              showToast(report['error']?.toString() ?? translate('Failed'));
            }
          }),
        ],
        onCancel: close);
  });
  return applied;
}

/// True when `text` is a provisioning payload the importer understands.
bool isConfigPayload(String text) =>
    text.startsWith('${bind.mainUriPrefixSync()}config/') ||
    text.startsWith('config=');

Map<String, dynamic> _json(String text) {
  try {
    final value = jsonDecode(text);
    if (value is Map<String, dynamic>) return value;
  } catch (_) {}
  return {'ok': false};
}
