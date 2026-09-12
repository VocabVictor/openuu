part of 'ab_model.dart';

class LegacyAb extends BaseAb {
  bool get emtpy => peers.isEmpty && tags.isEmpty;
  // licensedDevices is obtained from personal ab, shared ab restrict it in server
  var licensedDevices = 0;

  LegacyAb();

  @override
  AbProfile? sharedProfile() {
    return null;
  }

  @override
  void setSharedProfile(AbProfile? profile) {}

  @override
  bool canWrite() {
    return true;
  }

  @override
  bool fullControl() {
    return true;
  }

  @override
  bool isFull() {
    return licensedDevices > 0 && peers.length >= licensedDevices;
  }

  @override
  String name() {
    return _legacyAddressBookName;
  }

  @override
  Future<bool> pullAbImpl({quiet = false}) async {
    bool ret = false;
    final api = "${await bind.mainGetApiServer()}/api/ab";
    int? statusCode;
    try {
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      authHeaders['Accept-Encoding'] = "gzip";
      final resp = await http.get(Uri.parse(api), headers: authHeaders);
      statusCode = resp.statusCode;
      if (resp.body.toLowerCase() == "null") {
        // normal reply, empty ab return null
        tags.clear();
        tagColors.clear();
        peers.clear();
      } else if (resp.body.isNotEmpty) {
        Map<String, dynamic> json =
            _jsonDecodeRespMap(decode_http_response(resp), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        } else if (json.containsKey('data')) {
          try {
            licensedDevices = json['licensed_devices'];
            // ignore: empty_catches
          } catch (e) {}
          final data = jsonDecode(json['data']);
          if (data != null) {
            _deserialize(data);
          }
          ret = true;
        }
      }
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
    return ret;
  }

  Future<bool> pushAb(
      {bool toastIfFail = true, bool toastIfSucc = true}) async {
    debugPrint("pushAb: toastIfFail:$toastIfFail, toastIfSucc:$toastIfSucc");
    if (!gFFI.userModel.isLogin) return false;
    pushError.value = '';
    bool ret = false;
    try {
      //https: //stackoverflow.com/questions/68249333/flutter-getx-updating-item-in-children-list-is-not-reactive
      peers.refresh();
      final api = "${await bind.mainGetApiServer()}/api/ab";
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      final body = jsonEncode({"data": jsonEncode(_serialize())});
      http.Response resp =
          await http.post(Uri.parse(api), headers: authHeaders, body: body);
      if (resp.statusCode == 200 &&
          (resp.body.isEmpty || resp.body.toLowerCase() == 'null')) {
        ret = true;
      } else {
        Map<String, dynamic> json =
            _jsonDecodeRespMap(decode_http_response(resp), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        } else if (resp.statusCode == 200) {
          ret = true;
        } else {
          throw 'HTTP ${resp.statusCode}';
        }
      }
    } catch (e) {
      pushError.value =
          '${translate('push_ab_failed_tip')}: ${translate(e.toString())}';
    }

    if (!ret && toastIfFail) {
      BotToast.showText(contentColor: Colors.red, text: pushError.value);
    }
    if (ret && toastIfSucc) {
      showToast(translate('Successful'));
    }
    return ret;
  }

// #region Peer
  @override
  Future<String?> addPeers(List<Map<String, dynamic>> ps) async {
    bool full = false;
    for (var p in ps) {
      if (!isFull()) {
        p.remove('password'); // legacy ab ignore password
        final index = peers.indexWhere((e) => e.id == p['id']);
        if (index >= 0) {
          _merge(Peer.fromJson(p), peers[index]);
          _mergePeerFromGroup(peers[index]);
        } else {
          peers.add(Peer.fromJson(p));
        }
      } else {
        full = true;
        break;
      }
    }
    if (!await pushAb()) {
      return "Failed to push to server";
    } else if (full) {
      return translate("exceed_max_devices");
    } else {
      return null;
    }
  }

  _mergePeerFromGroup(Peer p) {
    final g = gFFI.groupModel.peers.firstWhereOrNull((e) => p.id == e.id);
    if (g == null) return;
    if (p.username.isEmpty) {
      p.username = g.username;
    }
    if (p.hostname.isEmpty) {
      p.hostname = g.hostname;
    }
    if (p.platform.isEmpty) {
      p.platform = g.platform;
    }
  }

  @override
  Future<bool> changeTagForPeers(List<String> ids, List<dynamic> tags) async {
    peers.map((e) {
      if (ids.contains(e.id)) {
        e.tags = tags;
      }
    }).toList();
    return await pushAb();
  }

  @override
  Future<bool> changeAlias({required String id, required String alias}) async {
    final it = peers.where((element) => element.id == id);
    if (it.isEmpty) {
      return false;
    }
    it.first.alias = alias;
    return await pushAb();
  }

  @override
  Future<bool> changeNote({required String id, required String note}) async {
    // no need to implement
    return false;
  }

  @override
  Future<bool> changeSharedPassword(String id, String password) async {
    // no need to implement
    return false;
  }

  @override
  Future<void> syncFromRecent(List<Peer> recents) async {
    bool peerSyncEqual(Peer a, Peer b) {
      return a.hash == b.hash &&
          a.username == b.username &&
          a.platform == b.platform &&
          a.hostname == b.hostname &&
          a.alias == b.alias;
    }

    bool needSync = false;
    for (var i = 0; i < recents.length; i++) {
      var r = recents[i];
      var index = peers.indexWhere((e) => e.id == r.id);
      if (index < 0) {
        if (!isFull()) {
          peers.add(r);
          needSync = true;
        }
      } else {
        Peer old = Peer.copy(peers[index]);
        _merge(r, peers[index]);
        if (!peerSyncEqual(peers[index], old)) {
          needSync = true;
        }
      }
    }
    if (needSync) {
      await pushAb(toastIfSucc: false, toastIfFail: false);
      gFFI.abModel._refreshTab();
    }
    // Pull cannot be used for sync to avoid cyclic sync.
  }

  void _merge(Peer r, Peer p) {
    p.hash = r.hash.isEmpty ? p.hash : r.hash;
    p.username = r.username.isEmpty ? p.username : r.username;
    p.hostname = r.hostname.isEmpty ? p.hostname : r.hostname;
    p.platform = r.platform.isEmpty ? p.platform : r.platform;
    p.alias = p.alias.isEmpty ? r.alias : p.alias;
  }

  @override
  Future<bool> changePersonalHashPassword(String id, String hash) async {
    bool changed = false;
    final it = peers.where((element) => element.id == id);
    if (it.isNotEmpty) {
      if (it.first.hash != hash) {
        it.first.hash = hash;
        changed = true;
      }
    }
    if (changed) {
      return await pushAb(toastIfSucc: false, toastIfFail: false);
    }
    return true;
  }

  @override
  Future<bool> deletePeers(List<String> ids) async {
    peers.removeWhere((e) => ids.contains(e.id));
    return await pushAb();
  }
// #endregion

// #region Tag
  @override
  Future<bool> addTags(
      List<String> tagList, Map<String, int> tagColorMap) async {
    for (var e in tagList) {
      if (!tagContainBy(e)) {
        tags.add(e);
      }
      if (tagColors[e] == null) {
        tagColors[e] = str2color2(e, existing: tagColors.values.toList()).value;
      }
    }
    return await pushAb();
  }

  @override
  Future<bool> renameTag(String oldTag, String newTag) async {
    if (tags.contains(newTag)) {
      BotToast.showText(
          contentColor: Colors.red, text: 'Tag $newTag already exists');
      return false;
    }
    tags.value = tags.map((e) {
      if (e == oldTag) {
        return newTag;
      } else {
        return e;
      }
    }).toList();
    for (var peer in peers) {
      peer.tags = peer.tags.map((e) {
        if (e == oldTag) {
          return newTag;
        } else {
          return e;
        }
      }).toList();
    }
    int? oldColor = tagColors[oldTag];
    if (oldColor != null) {
      tagColors.remove(oldTag);
      tagColors.addAll({newTag: oldColor});
    }
    return await pushAb();
  }

  @override
  Future<bool> setTagColor(String tag, Color color) async {
    if (tags.contains(tag)) {
      tagColors[tag] = color.value;
    }
    return await pushAb();
  }

  @override
  Future<bool> deleteTag(String tag) async {
    gFFI.abModel.selectedTags.remove(tag);
    tags.removeWhere((element) => element == tag);
    tagColors.remove(tag);
    for (var peer in peers) {
      if (peer.tags.isEmpty) {
        continue;
      }
      if (peer.tags.contains(tag)) {
        peer.tags.remove(tag);
      }
    }
    return await pushAb();
  }

// #endregion

  Map<String, dynamic> _serialize() {
    final peersJsonData =
        peers.map((e) => e.toCustomJson(includingHash: true)).toList();
    for (var e in tags) {
      if (tagColors[e] == null) {
        tagColors[e] = str2color2(e, existing: tagColors.values.toList()).value;
      }
    }
    final tagColorJsonData = jsonEncode(tagColors);
    return {
      "tags": tags,
      "peers": peersJsonData,
      "tag_colors": tagColorJsonData
    };
  }

  _deserialize(dynamic data) {
    if (data == null) return;
    final oldOnlineIDs = peers.where((e) => e.online).map((e) => e.id).toList();
    tags.clear();
    tagColors.clear();
    peers.clear();
    if (data['tags'] is List) {
      tags.value = (data['tags'] as List).map((e) => e.toString()).toList();
    }
    if (data['peers'] is List) {
      for (final peer in data['peers']) {
        peers.add(Peer.fromJson(peer));
      }
    }
    if (isFull()) {
      peers.removeRange(licensedDevices, peers.length);
    }
    // restore online
    peers
        .where((e) => oldOnlineIDs.contains(e.id))
        .map((e) => e.online = true)
        .toList();
    if (data['tag_colors'] is String) {
      Map<String, dynamic> map = jsonDecode(data['tag_colors']);
      tagColors.value = Map<String, int>.from(map);
    }
    // add color to tag
    final tagsWithoutColor =
        tags.toList().where((e) => !tagColors.containsKey(e)).toList();
    for (var t in tagsWithoutColor) {
      tagColors[t] = str2color2(t, existing: tagColors.values.toList()).value;
    }
  }
}
