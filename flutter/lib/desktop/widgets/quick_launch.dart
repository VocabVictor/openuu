import 'dart:convert';
import 'package:flutter/material.dart';
import '../../models/platform_model.dart';
import '../../models/model.dart';
import '../../models/quick_launch_model.dart';

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
  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    return Container(width: double.infinity, padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(border: Border.all(color: const Color(0xffdce2e7)), borderRadius: BorderRadius.circular(6)),
      child: Wrap(spacing: 12, runSpacing: 12, children: [
        for (var i = 0; i < apps.length; i++) SizedBox(width: 132, child: Column(children: [
          TextButton(onPressed: () { final app = Map<String, dynamic>.from(apps[i])..remove('icon'); widget.onOpen(jsonEncode(app)); }, child: Column(children: [
            _appIcon(apps[i]), const SizedBox(height: 8),
            Text(apps[i]['name'] ?? '', maxLines: 2, overflow: TextOverflow.ellipsis)])),
          PopupMenuButton<String>(tooltip: zh ? '管理快捷方式' : 'Manage shortcut',
            onSelected: (value) async {
              if (value == 'remove') { apps.removeAt(i); }
              if (value == 'up' && i > 0) { final item = apps.removeAt(i); apps.insert(i - 1, item); }
              if (value == 'rename') {
                final controller = TextEditingController(text: apps[i]['name']);
                final name = await showDialog<String>(context: context, builder: (ctx) => AlertDialog(
                  title: Text(zh ? '重命名' : 'Rename'), content: TextField(controller: controller),
                  actions: [TextButton(onPressed: () => Navigator.pop(ctx, controller.text.trim()), child: const Text('OK'))]));
                controller.dispose();
                if (name != null && name.isNotEmpty) apps[i]['name'] = name;
              }
              await _save(widget.peer, apps); if (mounted) setState(() {});
            }, itemBuilder: (_) => [
              PopupMenuItem(value: 'rename', child: Text(zh ? '重命名' : 'Rename')),
              PopupMenuItem(value: 'up', child: Text(zh ? '向前移动' : 'Move earlier')),
              PopupMenuItem(value: 'remove', child: Text(zh ? '移除快捷方式' : 'Remove shortcut')),
            ])])),
        SizedBox(width: 112, height: 110, child: TextButton(
          onPressed: () => widget.onOpen(''), child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
            const Icon(Icons.add_circle_outline, size: 34), const SizedBox(height: 8), Text(zh ? '添加' : 'Add')]))),
        IconButton(onPressed: _reload, tooltip: zh ? '刷新快捷方式' : 'Refresh shortcuts', icon: const Icon(Icons.refresh, size: 18)),
      ]));
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
  bool get zh => Localizations.localeOf(context).languageCode == 'zh';
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
    if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(zh ? '已添加快捷方式' : 'Shortcut added')));
  }
  Future<void> custom() async {
    final path = TextEditingController(); final args = TextEditingController();
    final app = await showDialog<Map<String, dynamic>>(context: context, builder: (ctx) => AlertDialog(
      title: Text(zh ? '手动添加程序' : 'Add executable'),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        TextField(controller: path, decoration: InputDecoration(labelText: zh ? '远端程序绝对路径' : 'Remote executable absolute path')),
        TextField(controller: args, minLines: 2, maxLines: 4, decoration: InputDecoration(labelText: zh ? '启动参数（每行一个，可留空）' : 'Arguments (one per line, optional)'))]),
      actions: [TextButton(onPressed: () => Navigator.pop(ctx), child: Text(zh ? '取消' : 'Cancel')),
        TextButton(onPressed: () {
          try { final values = args.text.split('\n').where((s) => s.isNotEmpty).toList(); if (path.text.trim().isEmpty) return;
            Navigator.pop(ctx, {'id': path.text.trim(), 'name': path.text.trim().split(RegExp(r'[/\\]')).last,
              'custom': true, 'arguments': values});
          } catch (_) { ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(zh ? '请检查程序路径和启动参数' : 'Check the application path and arguments'))); }
        }, child: Text(zh ? '添加' : 'Add'))]));
    path.dispose(); args.dispose(); if (app != null) await add(app);
  }
  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(zh ? '快速启动' : 'Quick launch'),
    content: SizedBox(width: 600, height: MediaQuery.of(context).size.height * .55, child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      if (user.isNotEmpty) Text('${zh ? '当前桌面用户' : 'Desktop user'}: $user · $session'),
      if (error.isNotEmpty) Padding(padding: const EdgeInsets.all(8), child: Text(error, style: const TextStyle(color: Colors.red))),
      if (busy) const Expanded(child: Center(child: CircularProgressIndicator()))
      else ...[
        TextField(onChanged: (s) => setState(() => query = s), decoration: InputDecoration(hintText: zh ? '搜索应用' : 'Search applications', prefixIcon: const Icon(Icons.search))),
        Expanded(child: ListView(children: [for (final app in apps.where((a) => (a['name'] as String).toLowerCase().contains(query.toLowerCase())))
          ListTile(leading: const Icon(Icons.apps), title: Text(app['name']),
            trailing: IconButton(tooltip: zh ? '添加快捷方式' : 'Add shortcut', icon: const Icon(Icons.add), onPressed: () => add(app)),
            onTap: () => launch({...app, 'session': session}))])),
      ]]),),
    actions: [if (!busy && session.isNotEmpty) TextButton(onPressed: custom, child: Text(zh ? '手动添加' : 'Add executable')),
      TextButton(onPressed: () => Navigator.pop(context), child: Text(zh ? '关闭' : 'Close'))]);
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
