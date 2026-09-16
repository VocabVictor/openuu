import 'dart:async';
import 'dart:convert';

class QuickLaunchRequests {
  static int _serial = 0;
  static final Map<String, String> _scopes = {};
  static final Map<String, Completer<Map<String, dynamic>>> _pending = {};

  static Future<Map<String, dynamic>> send(
      Future<void> Function(String) transport, Map<String, dynamic> request, {String scope = ''}) async {
    final id = '${DateTime.now().microsecondsSinceEpoch}-${_serial++}';
    final result = Completer<Map<String, dynamic>>();
    _pending[id] = result;
    _scopes[id] = scope;
    try {
      final raw = jsonEncode({...request, 'request_id': id});
      if (utf8.encode(raw).length > 32768) throw const FormatException('Quick launch request is too large.');
      await transport(raw);
      return await result.future.timeout(const Duration(seconds: 20),
          onTimeout: () => throw TimeoutException('Remote quick launch is unavailable or the request timed out.'));
    } finally { _pending.remove(id); _scopes.remove(id); }
  }

  static void receive(String raw, {String scope = ''}) {
    try {
      final response = jsonDecode(raw) as Map<String, dynamic>;
      if (_scopes[response['request_id']] != scope) return;
      final waiter = _pending[response['request_id']];
      if (waiter == null || waiter.isCompleted) return;
      if (response['error'] != null) {
        waiter.completeError(StateError(response['error'].toString()));
      } else {
        waiter.complete(Map<String, dynamic>.from(response['data']));
      }
    } catch (_) { /* Ignore malformed replies; pending requests still time out. */ }
  }
}
