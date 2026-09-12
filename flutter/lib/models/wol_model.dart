import '../common.dart';
import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'platform_model.dart';
import 'user_model.dart';

class WolModel {
  Timer? _timer;
  bool _busy = false;
  bool _stopped = false;

  static Future<Map<String, dynamic>> request(String action, Map<String, dynamic> body) async {
    final token = bind.mainGetLocalOption(key: 'access_token');
    if (token.isEmpty) throw StateError('Login required');
    final response = await http.post(Uri.parse('${UserModel.accountServer()}/api/wol/$action'),
      headers: {'Authorization': 'Bearer $token', 'Content-Type': 'application/json'},
      body: jsonEncode(body)).timeout(const Duration(seconds: 5));
    if (response.statusCode != 200) throw StateError('Wake service unavailable (${response.statusCode})');
    return jsonDecode(response.body) as Map<String, dynamic>;
  }

  void start() {
    _stopped = false;
    _timer = Timer.periodic(const Duration(seconds: 10), (_) => _poll());
    _poll();
  }

  void dispose() { _stopped = true; _timer?.cancel(); }

  Future<void> _poll() async {
    if (_busy || _stopped) return;
    _busy = true;
    try {
      final token = bind.mainGetLocalOption(key: 'access_token');
      if (token.isEmpty) return;
      final id = await bind.mainGetMyId();
      final peers = gFFI.lanPeersModel.peers.map((p) => p.id).where((p) => p.isNotEmpty).take(256).toList();
      final result = await request('poll', {'id': id, 'peers': peers});
      if (_stopped || bind.mainGetLocalOption(key: 'access_token') != token) return;
      for (final target in (result['targets'] as List? ?? [])) {
        if (target is String && peers.contains(target)) await bind.mainWol(id: target);
      }
    } catch (e) {
      debugPrint('Wake heartbeat unavailable: ${e.runtimeType}');
    } finally { _busy = false; }
  }
}
