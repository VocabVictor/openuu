import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'globals.dart';
import 'widgets_misc.dart';
import 'package:flutter_hbb/models/user_model.dart';

class ServerConfig {
  late String idServer;
  late String relayServer;
  late String apiServer;
  late String key;

  ServerConfig(
      {String? idServer, String? relayServer, String? apiServer, String? key}) {
    this.idServer = idServer?.trim() ?? '';
    this.relayServer = relayServer?.trim() ?? '';
    this.apiServer = apiServer?.trim() ?? '';
    this.key = key?.trim() ?? '';
  }

  /// decode from shared string (from user shared or rustdesk-server generated)
  /// also see [encode]
  /// throw when decoding failure
  ServerConfig.decode(String msg) {
    var json = {};
    try {
      // back compatible
      json = jsonDecode(msg);
    } catch (err) {
      final input = msg.split('').reversed.join('');
      final bytes = base64Decode(base64.normalize(input));
      json = jsonDecode(utf8.decode(bytes, allowMalformed: true));
    }
    idServer = json['host'] ?? '';
    relayServer = json['relay'] ?? '';
    apiServer = json['api'] ?? '';
    key = json['key'] ?? '';
  }

  /// encode to shared string
  /// also see [ServerConfig.decode]
  String encode() {
    Map<String, String> config = {};
    config['host'] = idServer.trim();
    config['relay'] = relayServer.trim();
    config['api'] = apiServer.trim();
    config['key'] = key.trim();
    return base64UrlEncode(Uint8List.fromList(jsonEncode(config).codeUnits))
        .split('')
        .reversed
        .join();
  }

  /// from local options
  ServerConfig.fromOptions(Map<String, dynamic> options)
      : idServer = options['custom-rendezvous-server'] ?? "",
        relayServer = options['relay-server'] ?? "",
        apiServer = options['api-server'] ?? "",
        key = options['key'] ?? "";
}

importConfig(List<TextEditingController>? controllers, List<RxString>? errMsgs,
    String? text) {
  text = text?.trim();
  if (text != null && text.isNotEmpty) {
    try {
      final sc = ServerConfig.decode(text);
      if (isWeb || isIOS) {
        sc.relayServer = '';
      }
      if (sc.idServer.isNotEmpty) {
        Future<bool> success = setServerConfig(controllers, errMsgs, sc);
        success.then((value) {
          if (value) {
            showToast(translate('Import server configuration successfully'));
          } else {
            showToast(translate('Invalid server configuration'));
          }
        });
      } else {
        showToast(translate('Invalid server configuration'));
      }
      return sc;
    } catch (e) {
      showToast(translate('Invalid server configuration'));
    }
  } else {
    showToast(translate('Clipboard is empty'));
  }
}

Future<bool> setServerConfig(
  List<TextEditingController>? controllers,
  List<RxString>? errMsgs,
  ServerConfig config,
) async {
  String removeEndSlash(String input) {
    if (input.endsWith('/')) {
      return input.substring(0, input.length - 1);
    }
    return input;
  }

  config.idServer = removeEndSlash(config.idServer.trim());
  config.relayServer = removeEndSlash(config.relayServer.trim());
  config.apiServer = removeEndSlash(config.apiServer.trim());
  config.key = config.key.trim();
  if (controllers != null) {
    controllers[0].text = config.idServer;
    controllers[1].text = config.relayServer;
    controllers[2].text = config.apiServer;
    controllers[3].text = config.key;
  }
  // id
  if (config.idServer.isNotEmpty && errMsgs != null) {
    errMsgs[0].value = translate(await bind.mainTestIfValidServer(
        server: config.idServer, testWithProxy: true));
    if (errMsgs[0].isNotEmpty) {
      return false;
    }
  }
  // relay
  if (config.relayServer.isNotEmpty && errMsgs != null) {
    errMsgs[1].value = translate(await bind.mainTestIfValidServer(
        server: config.relayServer, testWithProxy: true));
    if (errMsgs[1].isNotEmpty) {
      return false;
    }
  }
  // api
  if (config.apiServer.isNotEmpty && errMsgs != null) {
    if (!config.apiServer.startsWith('http://') &&
        !config.apiServer.startsWith('https://')) {
      errMsgs[2].value =
          '${translate("API Server")}: ${translate("invalid_http")}';
      return false;
    }
  }
  final oldApiServer = await bind.mainGetApiServer();

  // should set one by one
  await bind.mainSetOption(
      key: 'custom-rendezvous-server', value: config.idServer);
  await bind.mainSetOption(key: 'relay-server', value: config.relayServer);
  await bind.mainSetOption(key: 'api-server', value: config.apiServer);
  await bind.mainSetOption(key: 'key', value: config.key);
  final newApiServer = await bind.mainGetApiServer();
  if (oldApiServer.isNotEmpty &&
      oldApiServer != newApiServer &&
      gFFI.userModel.isLogin) {
    gFFI.userModel.logOut(apiServer: oldApiServer);
  }
  return true;
}
