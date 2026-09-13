import 'dart:convert';
import 'package:flutter/material.dart';
import '../../models/platform_model.dart';
import '../../models/model.dart';
import '../../models/quick_launch_model.dart';
import '../../common.dart';
import 'ui_tokens.dart';

const _shortcutKey = 'quick-launch-items';
Future<List<Map<String, dynamic>>> _load(String peer) async {
  try {
    final raw = await bind.mainGetPeerOption(id: peer, key: _shortcutKey);
    if (raw.isEmpty) return [];
    return (jsonDecode(raw) as List).map((e) => Map<String, dynamic>.from(e)).toList();
  } catch (_) { return []; }
}
Future<void> _save(String peer, List<Map<String, dynamic>> apps) =>
  bind.mainSetPeerOption(id: peer, key: _shortcutKey, value: jsonEncode(apps));

class QuickLaunchPanel extends StatefulWidget {
  final String peer;
  final void Function(String) onOpen;
  const QuickLaunchPanel({super.key, required this.peer, required this.onOpen});
  @override
  State<QuickLaunchPanel> createState() => _QuickLaunchPanelState();
}
class _QuickLaunchPanelState extends State<QuickLaunchPanel> {
  List<Map<String, dynamic>> apps = [];
  @override
  void initState() { super.initState(); _reload(); }
  Future<void> _reload() async {
    final loaded = await _load(widget.peer);
    if (mounted) setState(() => apps = loaded);
  }
  Widget _header() => SizedBox(
        height: UiSpace.sectionCardHeaderHeight,
        child: Row(children: [
          Expanded(child: Text(translate('Quick launch'), style: UiType.sectionTitle)),
          IconButton(
              onPressed: _reload,
              tooltip: translate('Refresh shortcuts'),
              iconSize: UiSpace.rowActionIconSize,
              constraints: const BoxConstraints.tightFor(
                  width: UiSpace.rowActionHitSize,
                  height: UiSpace.rowActionHitSize),
              padding: EdgeInsets.zero,
              icon: Icon(Icons.refresh, color: UiColor.of(context).muted)),
        ]),
      );

  /// An empty panel is one button and no explanation of what it adds, so it
  /// says what a shortcut is for before offering to make one.
  Widget _empty() => Padding(
        padding: const EdgeInsets.only(bottom: UiSpace.s3),
        child: Text(translate('quick-launch-empty-tip'), style: UiType.caption),
      );

  @override
  Widget build(BuildContext context) {
    return Container(width: double.infinity,
      padding: const EdgeInsets.all(UiSpace.sectionCardPadding),
      decoration: BoxDecoration(
        border: Border.all(color: UiColor.of(context).border),
        borderRadius: BorderRadius.circular(UiSpace.sectionCardRadius)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        _header(),
        if (apps.isEmpty) _empty(),
        Wrap(spacing: 12, runSpacing: 12, children: [
        for (var i = 0; i < apps.length; i++) SizedBox(width: 132, child: Column(children: [
          TextButton(onPressed: () { final app = Map<String, dynamic>.from(apps[i])..remove('icon'); widget.onOpen(jsonEncode(app)); }, child: Column(children: [
            _appIcon(apps[i]), const SizedBox(height: 8),
            Text(apps[i]['name'] ?? '', maxLines: 2, overflow: TextOverflow.ellipsis)])),
          PopupMenuButton<String>(tooltip: translate('Manage shortcut'),
            onSelected: (value) async {
              if (value == 'remove') { apps.removeAt(i); }
              if (value == 'up' && i > 0) { final item = apps.removeAt(i); apps.insert(i - 1, item); }
              if (value == 'rename') {
                final controller = TextEditingController(text: apps[i]['name']);
                final name = await showDialog<String>(context: context, builder: (ctx) => AlertDialog(
                  title: Text(translate('Rename')), content: TextField(controller: controller),
                  actions: [TextButton(onPressed: () => Navigator.pop(ctx, controller.text.trim()), child: const Text('OK'))]));
                controller.dispose();
                if (name != null && name.isNotEmpty) apps[i]['name'] = name;
              }
              await _save(widget.peer, apps); if (mounted) setState(() {});
            }, itemBuilder: (_) => [
              PopupMenuItem(value: 'rename', child: Text(translate('Rename'))),
              PopupMenuItem(value: 'up', child: Text(translate('Move earlier'))),
              PopupMenuItem(value: 'remove', child: Text(translate('Remove shortcut'))),
            ])])),
        SizedBox(width: 112, height: 110, child: TextButton(
          onPressed: () => widget.onOpen(''), child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
            const Icon(Icons.add_circle_outline, size: 34), const SizedBox(height: 8), Text(translate('Add'))]))),
      ])]));
  }
}

Future<void> showQuickLaunch(BuildContext context, FFI ffi, String initial) =>
  showDialog<void>(context: context, builder: (_) => _QuickLaunchDialog(ffi: ffi, initial: initial));

class _QuickLaunchDialog extends StatefulWidget {
  final FFI ffi;
  final String initial;
  const _QuickLaunchDialog({required this.ffi, required this.initial});
  @override
  State<_QuickLaunchDialog> createState() => _QuickLaunchDialogState();
}
class _QuickLaunchDialogState extends State<_QuickLaunchDialog> {
  List<Map<String, dynamic>> apps = [];
  String session = '', user = '', error = '', query = '';
  bool busy = true;
  Future<Map<String, dynamic>> request(Map<String, dynamic> data) {
    if (widget.ffi.viewOnlySession || widget.ffi.ffiModel.viewOnly) {
      return Future.error(StateError('Quick launch is unavailable in view-only mode.'));
    }
    if (!widget.ffi.ffiModel.pi.features.quickLaunch) {
      return Future.error(StateError('The remote client does not support quick launch. Update both clients first.'));
    }
    return QuickLaunchRequests.send((raw) => bind.sessionPeerOption(
      sessionId: widget.ffi.sessionId, name: 'quick-launch-request', value: raw), data, scope: widget.ffi.sessionId.toString());
  }
  @override
  void initState() { super.initState(); load(); }
  Future<void> load() async {
    try {
      if (widget.initial.isNotEmpty) {
        final app = Map<String, dynamic>.from(jsonDecode(widget.initial));
        final desktop = await request({'operation': 'list'});
        if (app['user'] != null && app['user'] != desktop['user']) {
          throw StateError('Switch the remote desktop to the shortcut owner first.');
        }
        if (!mounted) return;
        session = desktop['session']; user = desktop['user'];
        await launch({...app, 'session': session});
        return;
      }
      final data = await request({'operation': 'list'});
      if (!mounted) return;
      setState(() { apps = (data['apps'] as List).map((a) => Map<String, dynamic>.from(a)).toList();
        session = data['session']; user = data['user']; busy = false; });
    } catch (e) { if (mounted) setState(() { error = '$e'; busy = false; }); }
  }
  Future<void> launch(Map<String, dynamic> app) async {
    setState(() { busy = true; error = ''; });
    try {
      await request({'operation': 'launch', 'session': app['session'] ?? session,
        'user': user, 'app_id': app['id'], 'arguments': app['arguments'] ?? [], 'custom': app['custom'] == true});
      if (mounted) Navigator.pop(context);
    } catch (e) { if (mounted) setState(() { error = '$e'; busy = false; }); }
  }
  Future<void> add(Map<String, dynamic> app) async {
    try {
      final image = await request({'operation': 'icon', 'app_id': app['id'], 'custom': app['custom'] == true, 'session': session, 'user': user});
      app = {...app, 'icon': image['icon']};
    } catch (_) { /* An unavailable icon must not prevent adding an application. */ }
    final saved = await _load(widget.ffi.id);
    saved.removeWhere((a) => a['id'] == app['id'] && a['session'] == session);
    saved.add({...app, 'session': session, 'user': user});
    await _save(widget.ffi.id, saved);
    if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(translate('Shortcut added'))));
  }
  Future<void> custom() async {
    final path = TextEditingController(); final args = TextEditingController();
    final app = await showDialog<Map<String, dynamic>>(context: context, builder: (ctx) => AlertDialog(
      title: Text(translate('Add executable')),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        TextField(controller: path, decoration: InputDecoration(labelText: translate('Remote executable absolute path'))),
        TextField(controller: args, minLines: 2, maxLines: 4, decoration: InputDecoration(labelText: translate('Arguments (one per line, optional)')))]),
      actions: [TextButton(onPressed: () => Navigator.pop(ctx), child: Text(translate('Cancel'))),
        TextButton(onPressed: () {
          try { final values = args.text.split('\n').where((s) => s.isNotEmpty).toList(); if (path.text.trim().isEmpty) return;
            Navigator.pop(ctx, {'id': path.text.trim(), 'name': path.text.trim().split(RegExp(r'[/\\]')).last,
              'custom': true, 'arguments': values});
          } catch (_) { ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(translate('Check the application path and arguments')))); }
        }, child: Text(translate('Add')))]));
    path.dispose(); args.dispose(); if (app != null) await add(app);
  }
  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(translate('Quick launch')),
    content: SizedBox(width: 600, height: MediaQuery.of(context).size.height * .55, child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      if (user.isNotEmpty) Text('${translate('Desktop user')}: $user · $session'),
      if (error.isNotEmpty) Padding(padding: const EdgeInsets.all(8), child: Text(error, style: TextStyle(color: UiColor.of(context).danger))),
      if (busy) const Expanded(child: Center(child: CircularProgressIndicator()))
      else ...[
        TextField(onChanged: (s) => setState(() => query = s), decoration: InputDecoration(hintText: translate('Search applications'), prefixIcon: const Icon(Icons.search))),
        Expanded(child: ListView(children: [for (final app in apps.where((a) => (a['name'] as String).toLowerCase().contains(query.toLowerCase())))
          ListTile(leading: const Icon(Icons.apps), title: Text(app['name']),
            trailing: IconButton(tooltip: translate('Add shortcut'), icon: const Icon(Icons.add), onPressed: () => add(app)),
            onTap: () => launch({...app, 'session': session}))])),
      ]]),),
    actions: [if (!busy && session.isNotEmpty) TextButton(onPressed: custom, child: Text(translate('Add executable'))),
      TextButton(onPressed: () => Navigator.pop(context), child: Text(translate('Close')))]);
}

Widget _appIcon(Map<String, dynamic> app) {
  try {
    if (app['icon'] is String && (app['icon'] as String).isNotEmpty) {
      return Image.memory(base64Decode(app['icon']), width: 32, height: 32,
        errorBuilder: (_, __, ___) => const Icon(Icons.apps_rounded, size: 30));
    }
  } catch (_) {}
  return const Icon(Icons.apps_rounded, size: 30);
}
