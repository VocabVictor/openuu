import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

const kOptionPinnedWindowsSession = 'pinned-windows-session';

/// Settings row that pins incoming connections to one Windows user's session.
///
/// Only meaningful on Windows when installed as a service; shown otherwise
/// with a note so the user knows why it is disabled.
class PinnedSessionSetting extends StatefulWidget {
  final bool enabled;
  final double leftMargin;

  const PinnedSessionSetting(
      {Key? key, required this.enabled, required this.leftMargin})
      : super(key: key);

  @override
  State<PinnedSessionSetting> createState() => _PinnedSessionSettingState();
}

class _PinnedSessionSettingState extends State<PinnedSessionSetting> {
  List<Map<String, String>> _sessions = [];

  @override
  void initState() {
    super.initState();
    _sessions = _loadSessions();
  }

  static List<Map<String, String>> _loadSessions() {
    try {
      final decoded = json.decode(bind.mainGetCommonSync(key: 'windows-sessions'));
      if (decoded is List) {
        return decoded
            .map((e) => {
                  'user': (e['user'] ?? '').toString(),
                  'name': (e['name'] ?? '').toString(),
                })
            .where((e) => e['user']!.isNotEmpty)
            .toList();
      }
    } catch (_) {}
    return [];
  }

  @override
  Widget build(BuildContext context) {
    if (!isWindows) return const Offstage();
    final installed = bind.mainIsInstalled();
    final enabled = widget.enabled && installed;
    final current = bind.mainGetOptionSync(key: kOptionPinnedWindowsSession);
    final keys = <String>[''];
    final values = <String>[translate('Follow the active session')];
    for (final s in _sessions) {
      if (keys.contains(s['user'])) continue;
      keys.add(s['user']!);
      values.add(s['name']!);
    }
    if (current.isNotEmpty && !keys.contains(current)) {
      keys.add(current);
      values.add('$current (${translate('Not logged in')})');
    }
    final tip = installed
        ? 'pinned-windows-session-tip'
        : 'pinned-windows-session-service-tip';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(translate('Pin incoming sessions to a Windows user'),
            style: TextStyle(color: disabledTextColor(context, enabled))),
        Text(translate(tip),
                style: TextStyle(
                    fontSize: 12,
                    color: disabledTextColor(context, enabled)))
            .marginOnly(top: 2, bottom: 6),
        ComboBox(
          enabled: enabled,
          keys: keys,
          values: values,
          initialKey: current,
          onChanged: (key) async {
            await bind.mainSetOption(
                key: kOptionPinnedWindowsSession, value: key);
            if (mounted) setState(() => _sessions = _loadSessions());
          },
        ),
      ],
    ).marginOnly(left: widget.leftMargin, top: 8);
  }
}
