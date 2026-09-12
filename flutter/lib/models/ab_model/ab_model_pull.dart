part of 'ab_model.dart';

extension AbModelPull on AbModel {
// #region ab
  /// Pulls the address book data from the server.
  ///
  /// If `force` is `ForcePullAb.listAndCurrent`, the function will pull the list of address books, current address book, and try initialize personal address book.
  /// If `force` is `ForcePullAb.current`, the function will only pull the current address book.
  /// If `quiet` is true, the function will not display any notifications or errors.
  Future<void> pullAb(
      {required ForcePullAb? force, required bool quiet}) async {
    if (bind.isDisableAb()) return;
    if (!gFFI.userModel.isLogin) return;
    if (gFFI.userModel.networkError.isNotEmpty) return;
    if (_pulling) return;
    if (force == null && _pulledOnce) {
      return;
    }
    _pulling = true;
    if (!quiet) {
      _listPullError.value = '';
      current.pullError.value = '';
    }
    try {
      await _pullAb(force: force, quiet: quiet);
      _refreshTab();
    } catch (_) {}
    _pulling = false;
    _pulledOnce = true;
  }

  Future<void> _pullAb(
      {required ForcePullAb? force, required bool quiet}) async {
    if (force == null && listInitialized && current.initialized) return;
    debugPrint("pullAb, force: $force, quiet: $quiet");
    if (!listInitialized || force == ForcePullAb.listAndCurrent) {
      try {
        // Read personal guid every time to avoid upgrading the server without closing the main window
        _personalAbGuid = null;
        // `true`: continue init. `false`: stop, error already recorded.
        if (!await _getPersonalAbGuid(quiet: quiet)) {
          return;
        }
        legacyMode.value = _personalAbGuid == null;
        if (!legacyMode.value && _maxPeerOneAb == 0) {
          await _getAbSettings(quiet: quiet);
        }
        if (_personalAbGuid != null) {
          debugPrint("pull ab list");
          List<AbProfile> abProfiles = List.empty(growable: true);
          abProfiles.add(AbProfile(_personalAbGuid!, _personalAddressBookName,
              gFFI.userModel.userName.value, null, ShareRule.read.value, null));
          // get all address book name
          await _getSharedAbProfiles(abProfiles, quiet: quiet);
          addressbooks.removeWhere((key, value) =>
              abProfiles.firstWhereOrNull((e) => e.name == key) == null);
          for (int i = 0; i < abProfiles.length; i++) {
            AbProfile p = abProfiles[i];
            if (addressbooks.containsKey(p.name)) {
              addressbooks[p.name]?.setSharedProfile(p);
            } else {
              addressbooks[p.name] = Ab(p, p.guid == _personalAbGuid);
            }
          }
        } else {
          // only legacy address book
          addressbooks
              .removeWhere((key, value) => key != _legacyAddressBookName);
          if (!addressbooks.containsKey(_legacyAddressBookName)) {
            addressbooks[_legacyAddressBookName] = LegacyAb();
          }
        }
        // set current address book name
        if (!listInitialized) {
          listInitialized = true;
          trySetCurrentToLast();
        }
        if (!addressbooks.containsKey(_currentName.value)) {
          setCurrentName(legacyMode.value
              ? _legacyAddressBookName
              : _personalAddressBookName);
        }
        // pull current address book
        await current.pullAb(quiet: quiet);
        // try initialize personal address book
        if (!current.isPersonal()) {
          final personalAb = addressbooks[_personalAddressBookName];
          if (personalAb != null && !personalAb.initialized) {
            await personalAb.pullAb(quiet: quiet);
          }
        }
      } catch (e) {
        debugPrint("pull ab list error: $e");
        _setListPullError(e, quiet: quiet);
      }
    } else if (listInitialized &&
        (!current.initialized || force == ForcePullAb.current)) {
      try {
        await current.pullAb(quiet: quiet);
      } catch (e) {
        debugPrint("pull current Ab error: $e");
      }
    }
    _callbackPeerUpdate();
    if (listInitialized && current.initialized) {
      _saveCache();
    }
  }

  void _setListPullError(Object err, {required bool quiet, int? statusCode}) {
    if (!quiet) {
      _listPullError.value =
          '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
    }
    if (statusCode == 401) {
      gFFI.userModel.reset(resetOther: true);
    }
  }

  Future<bool> _getAbSettings({required bool quiet}) async {
    int? statusCode;
    try {
      final api = "${await bind.mainGetApiServer()}/api/ab/settings";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      _setEmptyBody(headers);
      final resp = await http.post(Uri.parse(api), headers: headers);
      statusCode = resp.statusCode;
      if (statusCode == 404) {
        debugPrint("HTTP 404, api server doesn't support shared address book");
        return false;
      }
      Map<String, dynamic> json =
          _jsonDecodeRespMap(decode_http_response(resp), resp.statusCode);
      if (json.containsKey('error')) {
        throw json['error'];
      }
      if (statusCode != 200) {
        throw 'HTTP $statusCode';
      }
      _maxPeerOneAb = json['max_peer_one_ab'] ?? 0;
      return true;
    } catch (err) {
      debugPrint('get ab settings err: ${err.toString()}');
      _setListPullError(err, quiet: quiet, statusCode: statusCode);
    }
    return false;
  }

  /// Loads `/api/ab/personal`.
  /// Returns `true` to continue init, `false` to stop after a real error.
  Future<bool> _getPersonalAbGuid({required bool quiet}) async {
    int? statusCode;
    try {
      final api = "${await bind.mainGetApiServer()}/api/ab/personal";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      _setEmptyBody(headers);
      final resp = await http.post(Uri.parse(api), headers: headers);
      statusCode = resp.statusCode;
      if (statusCode == 404) {
        debugPrint("HTTP 404, current api server is legacy mode");
        // Old server: keep `_personalAbGuid` null and continue in legacy mode.
        return true;
      }
      Map<String, dynamic> json =
          _jsonDecodeRespMap(decode_http_response(resp), resp.statusCode);
      if (json.containsKey('error')) {
        throw json['error'];
      }
      if (statusCode != 200) {
        throw 'HTTP $statusCode';
      }
      _personalAbGuid = json['guid'];
      // New server: guid is available, continue in non-legacy mode.
      return true;
    } catch (err) {
      debugPrint('get personal ab err: ${err.toString()}');
      _setListPullError(err, quiet: quiet, statusCode: statusCode);
    }
    // Real error: stop the current pull.
    return false;
  }

  Future<bool> _getSharedAbProfiles(List<AbProfile> profiles,
      {required bool quiet}) async {
    final api = "${await bind.mainGetApiServer()}/api/ab/shared/profiles";
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
            });
        var headers = getHttpHeaders();
        headers['Content-Type'] = "application/json";
        _setEmptyBody(headers);
        final resp = await http.post(uri, headers: headers);
        statusCode = resp.statusCode;
        if (statusCode == 404) {
          debugPrint(
              "HTTP 404, api server doesn't support shared address book");
          return false;
        }
        Map<String, dynamic> json =
            _jsonDecodeRespMap(decode_http_response(resp), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        }
        if (statusCode != 200) {
          throw 'HTTP $statusCode';
        }
        if (json.containsKey('total')) {
          if (total == 0) total = json['total'];
          if (json.containsKey('data')) {
            final data = json['data'];
            if (data is List) {
              for (final profile in data) {
                final u = AbProfile.fromJson(profile);
                int index = profiles.indexWhere((e) => e.name == u.name);
                if (index < 0) {
                  profiles.add(u);
                } else {
                  profiles[index] = u;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      debugPrint('_getSharedAbProfiles err: ${err.toString()}');
      _setListPullError(err, quiet: quiet, statusCode: statusCode);
    }
    return false;
  }

// #endregion
}
