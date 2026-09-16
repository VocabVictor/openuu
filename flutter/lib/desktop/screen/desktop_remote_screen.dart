import 'package:flutter/material.dart';
import 'package:openuu/common.dart';
import 'package:openuu/desktop/pages/remote_tab.dart';
import 'package:openuu/models/platform.dart';
import 'package:openuu/models/state.dart';
import 'package:provider/provider.dart';

/// multi-tab desktop remote screen
class DesktopRemoteScreen extends StatelessWidget {
  final Map<String, dynamic> params;

  DesktopRemoteScreen({Key? key, required this.params}) : super(key: key) {
      bind.mainInitInputSource();
      stateGlobal.getInputSource(force: true);
  }

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: gFFI.ffiModel),
          ChangeNotifierProvider.value(value: gFFI.imageModel),
          ChangeNotifierProvider.value(value: gFFI.cursorModel),
          ChangeNotifierProvider.value(value: gFFI.canvasModel),
        ],
        child: Scaffold(
          // Set transparent background for padding the resize area out of the flutter view.
          // This allows the wallpaper goes through our resize area. (Linux only now).
          backgroundColor: isLinux ? Colors.transparent : null,
          body: ConnectionTabPage(
            params: params,
          ),
        ));
  }
}
