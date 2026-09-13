import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:uni_links/uni_links.dart';

import '../models/platform_model.dart';

import 'globals.dart';
import 'url_cmd_args.dart';
import 'windows_misc.dart';


/// Initialize uni links for macos/windows
///
/// [Availability]
/// initUniLinks should only be used on macos/windows.
/// we use dbus for linux currently.
Future<bool> initUniLinks() async {
  if (isLinux) {
    return false;
  }
  // check cold boot
  try {
    final initialLink = await getInitialLink();
    print("initialLink: $initialLink");
    if (initialLink == null || initialLink.isEmpty) {
      return false;
    }
    return handleUriLink(uriString: initialLink);
  } catch (err) {
    debugPrintStack(label: "$err");
    return false;
  }
}

/// Listen for uni links.
///
/// * handleByFlutter: Should uni links be handled by Flutter.
///
/// Returns a [StreamSubscription] which can listen the uni links.
StreamSubscription? listenUniLinks({handleByFlutter = true}) {
  if (isLinux) {
    return null;
  }

  final sub = uriLinkStream.listen((Uri? uri) {
    debugPrint("A uri was received: $uri. handleByFlutter $handleByFlutter");
    if (uri != null) {
      if (handleByFlutter) {
        handleUriLink(uri: uri);
      } else {
        bind.sendUrlScheme(url: uri.toString());
      }
    } else {
      print("uni listen error: uri is empty.");
    }
  }, onError: (err) {
    print("uni links error: $err");
  });
  return sub;
}

enum UriLinkType {
  remoteDesktop,
  fileTransfer,
  viewCamera,
  portForward,
  rdp,
  terminal,
}

setEnvTerminalAdmin() {
  bind.mainSetEnv(key: 'IS_TERMINAL_ADMIN', value: 'Y');
}

// uri link handler
bool handleUriLink({List<String>? cmdArgs, Uri? uri, String? uriString}) {
  List<String>? args;
  if (cmdArgs != null && cmdArgs.isNotEmpty) {
    args = cmdArgs;
    // rustdesk <uri link>
    if (args[0].startsWith(bind.mainUriPrefixSync())) {
      final uri = Uri.tryParse(args[0]);
      if (uri != null) {
        args = urlLinkToCmdArgs(uri);
      }
    }
  } else if (uri != null) {
    args = urlLinkToCmdArgs(uri);
  } else if (uriString != null) {
    final uri = Uri.tryParse(uriString);
    if (uri != null) {
      args = urlLinkToCmdArgs(uri);
    }
  }
  if (args == null) {
    return false;
  }

  if (args.isEmpty) {
    windowOnTop(null);
    return true;
  }

  UriLinkType? type;
  String? id;
  String? password;
  String? switchUuid;
  bool? forceRelay;
  for (int i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--connect':
      case '--play':
        type = UriLinkType.remoteDesktop;
        id = args[i + 1];
        i++;
        break;
      case '--file-transfer':
        type = UriLinkType.fileTransfer;
        id = args[i + 1];
        i++;
        break;
      case '--view-camera':
        type = UriLinkType.viewCamera;
        id = args[i + 1];
        i++;
        break;
      case '--port-forward':
        type = UriLinkType.portForward;
        id = args[i + 1];
        i++;
        break;
      case '--rdp':
        type = UriLinkType.rdp;
        id = args[i + 1];
        i++;
        break;
      case '--terminal':
        type = UriLinkType.terminal;
        id = args[i + 1];
        i++;
        break;
      case '--terminal-admin':
        setEnvTerminalAdmin();
        type = UriLinkType.terminal;
        id = args[i + 1];
        i++;
        break;
      case '--password':
        password = args[i + 1];
        i++;
        break;
      case '--switch_uuid':
        switchUuid = args[i + 1];
        i++;
        break;
      case '--relay':
        forceRelay = true;
        break;
      default:
        break;
    }
  }
  if (type != null && id != null) {
    switch (type) {
      case UriLinkType.remoteDesktop:
        Future.delayed(Duration.zero, () {
          rustDeskWinManager.newRemoteDesktop(id!,
              password: password,
              switchUuid: switchUuid,
              forceRelay: forceRelay);
        });
        break;
      case UriLinkType.fileTransfer:
        Future.delayed(Duration.zero, () {
          rustDeskWinManager.newFileTransfer(id!,
              password: password, forceRelay: forceRelay);
        });
        break;
      case UriLinkType.viewCamera:
        Future.delayed(Duration.zero, () {
          rustDeskWinManager.newViewCamera(id!,
              password: password, forceRelay: forceRelay);
        });
        break;
      case UriLinkType.portForward:
        Future.delayed(Duration.zero, () {
          rustDeskWinManager.newPortForward(id!, false,
              password: password, forceRelay: forceRelay);
        });
        break;
      case UriLinkType.rdp:
        Future.delayed(Duration.zero, () {
          rustDeskWinManager.newPortForward(id!, true,
              password: password, forceRelay: forceRelay);
        });
        break;
      case UriLinkType.terminal:
        Future.delayed(Duration.zero, () {
          rustDeskWinManager.newTerminal(id!,
              password: password, forceRelay: forceRelay);
        });
        break;
    }

    return true;
  }

  return false;
}
