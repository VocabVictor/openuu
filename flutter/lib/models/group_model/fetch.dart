part of 'group_model.dart';

extension GroupModelFetch on GroupModel {
  Future<bool> _getDeviceGroups(
      List<DeviceGroupPayload> tmpDeviceGroups) async {
    final api = "${await bind.mainGetApiServer()}/api/device-group/accessible";
    try {
      var uri0 = Uri.parse(api);
      final pageSize = 100;
      var total = 0;
      int current = 0;
      do {
        current += 1;
        var uri = Uri(
            scheme: uri0.scheme,
            host: uri0.host,
            path: uri0.path,
            port: uri0.port,
            queryParameters: {
              'current': current.toString(),
              'pageSize': pageSize.toString(),
            });
        final resp = await http.get(uri, headers: getHttpHeaders());
        _statusCode = resp.statusCode;
        Map<String, dynamic> json =
            _jsonDecodeResp(decode_http_response(resp), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        }
        if (resp.statusCode != 200) {
          throw 'HTTP ${resp.statusCode}';
        }
        if (json.containsKey('total')) {
          if (total == 0) total = json['total'];
          if (json.containsKey('data')) {
            final data = json['data'];
            if (data is List) {
              for (final user in data) {
                final u = DeviceGroupPayload.fromJson(user);
                int index = tmpDeviceGroups.indexWhere((e) => e.name == u.name);
                if (index < 0) {
                  tmpDeviceGroups.add(u);
                } else {
                  tmpDeviceGroups[index] = u;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      debugPrint('get accessible device groups: $err');
      // old hbbs doesn't support this api
      // groupLoadError.value =
      //     '${translate('pull_group_failed_tip')}: ${translate(err.toString())}';
    }
    return false;
  }

  Future<bool> _getUsers(List<UserPayload> tmpUsers) async {
    final api = "${await bind.mainGetApiServer()}/api/users";
    try {
      var uri0 = Uri.parse(api);
      final pageSize = 100;
      var total = 0;
      int current = 0;
      do {
        current += 1;
        var uri = Uri(
            scheme: uri0.scheme,
            host: uri0.host,
            path: uri0.path,
            port: uri0.port,
            queryParameters: {
              'current': current.toString(),
              'pageSize': pageSize.toString(),
              'accessible': '',
              'status': '1',
            });
        final resp = await http.get(uri, headers: getHttpHeaders());
        _statusCode = resp.statusCode;
        Map<String, dynamic> json =
            _jsonDecodeResp(decode_http_response(resp), resp.statusCode);
        if (json.containsKey('error')) {
          if (json['error'] == 'Admin required!' ||
              json['error']
                  .toString()
                  .contains('ambiguous column name: status')) {
            throw translate('upgrade_rustdesk_server_pro_to_{1.1.10}_tip');
          } else {
            throw json['error'];
          }
        }
        if (resp.statusCode != 200) {
          throw 'HTTP ${resp.statusCode}';
        }
        if (json.containsKey('total')) {
          if (total == 0) total = json['total'];
          if (json.containsKey('data')) {
            final data = json['data'];
            if (data is List) {
              for (final user in data) {
                final u = UserPayload.fromJson(user);
                int index = tmpUsers.indexWhere((e) => e.name == u.name);
                if (index < 0) {
                  tmpUsers.add(u);
                } else {
                  tmpUsers[index] = u;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      debugPrint('get accessible users: $err');
      groupLoadError.value =
          '${translate('pull_group_failed_tip')}: ${translate(err.toString())}';
    }
    return false;
  }

  Future<bool> _getPeers(List<Peer> tmpPeers) async {
    try {
      final api = "${await bind.mainGetApiServer()}/api/peers";
      var uri0 = Uri.parse(api);
      final pageSize = 100;
      var total = 0;
      int current = 0;
      do {
        current += 1;
        var queryParameters = {
          'current': current.toString(),
          'pageSize': pageSize.toString(),
          'accessible': '',
          'status': '1',
        };
        var uri = Uri(
            scheme: uri0.scheme,
            host: uri0.host,
            path: uri0.path,
            port: uri0.port,
            queryParameters: queryParameters);
        final resp = await http.get(uri, headers: getHttpHeaders());
        _statusCode = resp.statusCode;

        Map<String, dynamic> json =
            _jsonDecodeResp(decode_http_response(resp), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        }
        if (resp.statusCode != 200) {
          throw 'HTTP ${resp.statusCode}';
        }
        if (json.containsKey('total')) {
          if (total == 0) total = json['total'];
          if (json.containsKey('data')) {
            final data = json['data'];
            if (data is List) {
              for (final p in data) {
                final peerPayload = PeerPayload.fromJson(p);
                final peer = PeerPayload.toPeer(peerPayload);
                int index = tmpPeers.indexWhere((e) => e.id == peer.id);
                if (index < 0) {
                  tmpPeers.add(peer);
                } else {
                  tmpPeers[index] = peer;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      debugPrint('get accessible peers: $err');
      groupLoadError.value =
          '${translate('pull_group_failed_tip')}: ${translate(err.toString())}';
    }
    return false;
  }
}
