import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../consts.dart';
import '../models/platform_model.dart';

import 'connect.dart';
import 'ffi_options.dart';
import 'globals.dart';
import 'server_config.dart';
import 'uri_links.dart';
import 'widgets_misc.dart';

List<String>? urlLinkToCmdArgs(Uri uri) {
  String? command;
  String? id;
  final options = [
    "connect",
    "play",
    "file-transfer",
    "view-camera",
    "port-forward",
    "rdp",
    "terminal",
    "terminal-admin",
  ];
  if (uri.authority.isEmpty &&
      uri.path.split('').every((char) => char == '/')) {
    return [];
  } else if (uri.authority == "connection" && uri.path.startsWith("/new/")) {
    // For compatibility
    command = '--connect';
    id = uri.path.substring("/new/".length);
  } else if (uri.authority == "config") {
    if (isAndroid || isIOS) {
      final allowDeepLinkServerSettings =
          bind.mainGetBuildinOption(key: kOptionAllowDeepLinkServerSettings) ==
              'Y';
      if (!allowDeepLinkServerSettings) {
        debugPrint(
            "Ignore rustdesk://config because $kOptionAllowDeepLinkServerSettings is not enabled.");
        // Keep the user-facing error generic; detailed rejection reason is in debug logs.
        // Delay toast to avoid missing overlay during cold-start deeplink handling.
        Timer(Duration(seconds: 1), () {
          showToast(translate('Failed'));
        });
        return null;
      }
      final config = uri.path.substring("/".length);
      // add a timer to make showToast work
      Timer(Duration(seconds: 1), () {
        importConfig(null, null, config);
      });
    }
    return null;
  } else if (uri.authority == "password") {
    if (isAndroid || isIOS) {
      final allowDeepLinkPassword =
          bind.mainGetBuildinOption(key: kOptionAllowDeepLinkPassword) == 'Y';
      if (!allowDeepLinkPassword) {
        debugPrint(
            "Ignore rustdesk://password because $kOptionAllowDeepLinkPassword is not enabled.");
        // Keep the user-facing error generic; detailed rejection reason is in debug logs.
        // Delay toast to avoid missing overlay during cold-start deeplink handling.
        Timer(Duration(seconds: 1), () {
          showToast(translate('Failed'));
        });
        return null;
      }
      final password = uri.path.substring("/".length);
      if (password.isNotEmpty) {
        Timer(Duration(seconds: 1), () async {
          final ok =
              await bind.mainSetPermanentPasswordWithResult(password: password);
          showToast(translate(ok ? 'Successful' : 'Failed'));
        });
      }
    }
  } else if (options.contains(uri.authority)) {
    command = '--${uri.authority}';
    if (uri.path.length > 1) {
      id = uri.path.substring(1);
    }
  } else if (uri.authority.length > 2 &&
      (uri.path.length <= 1 ||
          (uri.path == '/r' || uri.path.startsWith('/r@')))) {
    // rustdesk://<connect-id>
    // rustdesk://<connect-id>/r
    // rustdesk://<connect-id>/r@<server>
    command = '--connect';
    id = uri.authority;
    if (uri.path.length > 1) {
      id = id + uri.path;
    }
  }

  var queryParameters =
      uri.queryParameters.map((k, v) => MapEntry(k.toLowerCase(), v));

  var key = queryParameters["key"];
  if (id != null) {
    if (key != null) {
      id = "$id?key=$key";
    }
  }

  if (isMobile && id != null) {
    final forceRelay = queryParameters["relay"] != null;
    final password = queryParameters["password"];

    // Determine connection type based on command
    if (command == '--file-transfer') {
      connect(Get.context!, id,
          isFileTransfer: true, forceRelay: forceRelay, password: password);
    } else if (command == '--view-camera') {
      connect(Get.context!, id,
          isViewCamera: true, forceRelay: forceRelay, password: password);
    } else if (command == '--terminal') {
      connect(Get.context!, id,
          isTerminal: true, forceRelay: forceRelay, password: password);
    } else if (command == 'terminal-admin') {
      setEnvTerminalAdmin();
      connect(Get.context!, id,
          isTerminal: true, forceRelay: forceRelay, password: password);
    } else {
      // Default to remote desktop for '--connect', '--play', or direct connection
      connect(Get.context!, id, forceRelay: forceRelay, password: password);
    }
    return null;
  }

  List<String> args = List.empty(growable: true);
  if (command != null && id != null) {
    args.add(command);
    args.add(id);
    var param = queryParameters;
    String? password = param["password"];
    if (password != null) args.addAll(['--password', password]);
    String? switch_uuid = param["switch_uuid"];
    if (switch_uuid != null) args.addAll(['--switch_uuid', switch_uuid]);
    if (param["relay"] != null) args.add("--relay");
    return args;
  }

  return null;
}
