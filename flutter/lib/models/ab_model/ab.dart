part of 'ab_model.dart';

class Ab extends BaseAb {
  AbProfile profile;
  late final bool personal;
  bool get emtpy => peers.isEmpty && tags.isEmpty;

  Ab(this.profile, this.personal);

  @override
  String name() {
    if (personal) {
      return _personalAddressBookName;
    } else {
      return profile.name;
    }
  }

  @override
  AbProfile? sharedProfile() {
    return profile;
  }

  @override
  void setSharedProfile(AbProfile profile) {
    this.profile = profile;
  }

  @override
  bool isFull() {
    return gFFI.abModel._maxPeerOneAb > 0 &&
        peers.length >= gFFI.abModel._maxPeerOneAb;
  }

  @override
  bool canWrite() {
    if (personal) {
      return true;
    } else {
      return profile.rule == ShareRule.readWrite.value ||
          profile.rule == ShareRule.fullControl.value;
    }
  }

  @override
  bool fullControl() {
    if (personal) {
      return true;
    } else {
      return profile.rule == ShareRule.fullControl.value;
    }
  }

  @override
  Future<bool> pullAbImpl({quiet = false}) async {
    bool ret = true;
    List<Peer> tmpPeers = [];
    if (!await _fetchPeers(tmpPeers, quiet: quiet)) {
      ret = false;
    }
    peers.value = tmpPeers;
    List<AbTag> tmpTags = [];
    if (!await _fetchTags(tmpTags, quiet: quiet)) {
      ret = false;
    }
    tags.value = tmpTags.map((e) => e.name).toList();
    Map<String, int> tmpTagColors = {};
    for (var t in tmpTags) {
      tmpTagColors[t.name] = t.color;
    }
    tagColors.value = tmpTagColors;
    return ret;
  }

  Future<bool> _fetchPeers(List<Peer> tmpPeers, {quiet = false}) async {
    final api = "${await bind.mainGetApiServer()}/api/ab/peers";
    int? statusCode;
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
              'ab': profile.guid,
            });
        var headers = getHttpHeaders();
        headers['Content-Type'] = "application/json";
        _setEmptyBody(headers);
        final resp = await http.post(uri, headers: headers);
        statusCode = resp.statusCode;
        Map<String, dynamic> json =
            _jsonDecodeRespMap(decode_http_response(resp), resp.statusCode);
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
              for (final profile in data) {
                final u = Peer.fromJson(profile);
                int index = tmpPeers.indexWhere((e) => e.id == u.id);
                if (index < 0) {
                  tmpPeers.add(u);
                } else {
                  tmpPeers[index] = u;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
      }
    } finally {
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(resetOther: true);
        }
      }
    }
    return false;
  }

  Future<bool> _fetchTags(List<AbTag> tmpTags, {quiet = false}) async {
    final api = "${await bind.mainGetApiServer()}/api/ab/tags/${profile.guid}";
    int? statusCode;
    try {
      var uri0 = Uri.parse(api);
      var uri = Uri(
        scheme: uri0.scheme,
        host: uri0.host,
        path: uri0.path,
        port: uri0.port,
      );
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      _setEmptyBody(headers);
      final resp = await http.post(uri, headers: headers);
      statusCode = resp.statusCode;
      List<dynamic> json =
          _jsonDecodeRespList(decode_http_response(resp), resp.statusCode);
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }

      for (final d in json) {
        final t = AbTag.fromJson(d);
        int index = tmpTags.indexWhere((e) => e.name == t.name);
        if (index < 0) {
          tmpTags.add(t);
        } else {
          tmpTags[index] = t;
        }
      }
      return true;
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
      }
    } finally {
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(resetOther: true);
        }
      }
    }
    return false;
  }

// #region Peers
  @override
  Future<String?> addPeers(List<Map<String, dynamic>> ps) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/add/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      for (var p in ps) {
        if (peers.firstWhereOrNull((e) => e.id == p['id']) != null) {
          continue;
        }
        if (isFull()) {
          return translate("exceed_max_devices");
        }
        if (personal) {
          removePassword(p);
        } else {
          removeHash(p);
        }
        String body = jsonEncode(p);
        final resp =
            await http.post(Uri.parse(api), headers: headers, body: body);
        final errMsg = _jsonDecodeActionResp(resp);
        if (errMsg.isNotEmpty) {
          return errMsg;
        }
      }
    } catch (err) {
      return err.toString();
    }
    return null;
  }

  @override
  Future<bool> changeTagForPeers(List<String> ids, List<dynamic> tags) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      var ret = true;
      for (var id in ids) {
        final body = jsonEncode({"id": id, "tags": tags});
        final resp =
            await http.put(Uri.parse(api), headers: headers, body: body);
        final errMsg = _jsonDecodeActionResp(resp);
        if (errMsg.isNotEmpty) {
          BotToast.showText(contentColor: Colors.red, text: errMsg);
          ret = false;
          break;
        }
      }
      return ret;
    } catch (err) {
      debugPrint('changeTagForPeers err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> changeAlias({required String id, required String alias}) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({"id": id, "alias": alias});
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('changeAlias err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> changeNote({required String id, required String note}) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({"id": id, "note": note});
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('changeNote err: ${err.toString()}');
      return false;
    }
  }

  Future<bool> _setPassword(Object bodyContent) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode(bodyContent);
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('changeSharedPassword err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> changePersonalHashPassword(String id, String hash) async {
    if (!personal) return false;
    if (!peers.any((e) => e.id == id)) return true;
    return await _setPassword({"id": id, "hash": hash});
  }

  @override
  Future<bool> changeSharedPassword(String id, String password) async {
    if (personal) return false;
    return await _setPassword({"id": id, "password": password});
  }

  @override
  Future<void> syncFromRecent(List<Peer> recents) async {
    bool uiUpdate = false;
    bool saveCache = false;
    final api =
        "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
    var headers = getHttpHeaders();
    headers['Content-Type'] = "application/json";

    Future<bool> trySyncOnePeer(Peer p, Peer r) async {
      var map = Map<String, String>.fromEntries([]);
      if (p.sameServer != true &&
          r.username.isNotEmpty &&
          p.username != r.username) {
        p.username = r.username;
        map['username'] = r.username;
      }
      if (p.sameServer != true &&
          r.hostname.isNotEmpty &&
          p.hostname != r.hostname) {
        p.hostname = r.hostname;
        map['hostname'] = r.hostname;
      }
      if (p.sameServer != true &&
          r.platform.isNotEmpty &&
          p.platform != r.platform) {
        p.platform = r.platform;
        map['platform'] = r.platform;
      }
      if (personal && r.hash.isNotEmpty && p.hash != r.hash) {
        p.hash = r.hash;
        map['hash'] = r.hash;
        saveCache = true;
      }
      if (map.isEmpty) {
        // no need to sync
        return false;
      }
      map['id'] = p.id;
      final body = jsonEncode(map);
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        debugPrint('syncOnePeer errMsg: $errMsg');
        return false;
      }
      uiUpdate = true;
      return true;
    }

    try {
      // Not add new peers because IDs that are not on the server can't be synced, then sync will happen every startup.
      for (var p in peers) {
        Peer? r = recents.firstWhereOrNull((e) => e.id == p.id);
        if (r != null) {
          await trySyncOnePeer(p, r);
        }
      }
      // Pull cannot be used for sync to avoid cyclic sync.
      if (uiUpdate && gFFI.abModel.currentName.value == profile.name) {
        peers.refresh();
      }
      if (saveCache) {
        gFFI.abModel._saveCache();
      }
    } catch (err) {
      debugPrint('syncFromRecent err: ${err.toString()}');
    }
  }

  @override
  Future<bool> deletePeers(List<String> ids) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode(ids);
      final resp =
          await http.delete(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('deletePeers err: ${err.toString()}');
      return false;
    }
  }
// #endregion

// #region Tags
  @override
  Future<bool> addTags(
      List<String> tagList, Map<String, int> tagColorMap) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/tag/add/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      for (var t in tagList) {
        final body = jsonEncode({
          "name": t,
          "color": tagColorMap[t] ??
              str2color2(t, existing: tagColors.values.toList()).value,
        });
        final resp =
            await http.post(Uri.parse(api), headers: headers, body: body);
        final errMsg = _jsonDecodeActionResp(resp);
        if (errMsg.isNotEmpty) {
          BotToast.showText(contentColor: Colors.red, text: errMsg);
          return false;
        }
      }
      return true;
    } catch (err) {
      debugPrint('addTags err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> renameTag(String oldTag, String newTag) async {
    if (tags.contains(newTag)) {
      BotToast.showText(
          contentColor: Colors.red, text: 'Tag $newTag already exists');
      return false;
    }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/tag/rename/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        "old": oldTag,
        "new": newTag,
      });
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('renameTag err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> setTagColor(String tag, Color color) async {
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/tag/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        "name": tag,
        "color": color.value,
      });
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('setTagColor err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> deleteTag(String tag) async {
    try {
      final api = "${await bind.mainGetApiServer()}/api/ab/tag/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode([tag]);
      final resp =
          await http.delete(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('deleteTag err: ${err.toString()}');
      return false;
    }
  }

// #endregion
}
