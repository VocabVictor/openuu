import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/toolbar.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import '../../../common.dart';
import '../../../common/widgets/overlay.dart';
import '../../../common/widgets/dialog.dart';
import '../../../common/widgets/remote_input.dart';
import '../../../models/input_model.dart';
import '../../../models/model.dart';
import '../../../models/platform_model.dart';
import '../../../utils/image.dart';
part 'actions.dart';
part 'body.dart';
part 'options.dart';

final initText = '1' * 1024;

// Workaround for Android (default input method, Microsoft SwiftKey keyboard) when using physical keyboard.
// When connecting a physical keyboard, `KeyEvent.physicalKey.usbHidUsage` are wrong is using Microsoft SwiftKey keyboard.
// https://github.com/flutter/flutter/issues/159384
// https://github.com/flutter/flutter/issues/159383
void _disableAndroidSoftKeyboard({bool? isKeyboardVisible}) {
  if (isAndroid) {
    if (isKeyboardVisible != true) {
      // `enable_soft_keyboard` will be set to `true` when clicking the keyboard icon, in `openKeyboard()`.
      gFFI.invokeMethod("enable_soft_keyboard", false);
    }
  }
}

class ViewCameraPage extends StatefulWidget {
  ViewCameraPage(
      {Key? key,
      required this.id,
      this.password,
      this.isSharedPassword,
      this.forceRelay})
      : super(key: key);

  final String id;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;

  @override
  State<ViewCameraPage> createState() => _ViewCameraPageState(id);
}

class _ViewCameraPageState extends State<ViewCameraPage>
    with WidgetsBindingObserver {
  void _setState(VoidCallback fn) => setState(fn);
  Timer? _timer;
  bool _showBar = true;
  bool _showGestureHelp = false;
  Orientation? _currentOrientation;
  double _viewInsetsBottom = 0;
  final _uniqueKey = UniqueKey();
  Timer? _timerDidChangeMetrics;

  final _blockableOverlayState = BlockableOverlayState();

  final keyboardVisibilityController = KeyboardVisibilityController();
  final FocusNode _mobileFocusNode = FocusNode();
  final FocusNode _physicalFocusNode = FocusNode();
  var _showEdit = false; // use soft keyboard

  InputModel get inputModel => gFFI.inputModel;
  SessionID get sessionId => gFFI.sessionId;

  final TextEditingController _textController =
      TextEditingController(text: initText);

  _ViewCameraPageState(String id) {
    initSharedStates(id);
    gFFI.chatModel.voiceCallStatus.value = VoiceCallStatus.notStarted;
    gFFI.dialogManager.loadMobileActionsOverlayVisible();
  }

  @override
  void initState() {
    super.initState();
    gFFI.ffiModel.updateEventListener(sessionId, widget.id);
    gFFI.start(
      widget.id,
      isViewCamera: true,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    WakelockManager.enable(_uniqueKey);
    _physicalFocusNode.requestFocus();
    gFFI.inputModel.listenToMouse(true);
    gFFI.qualityMonitorModel.checkShowQualityMonitor(sessionId);
    gFFI.chatModel
        .changeCurrentKey(MessageKey(widget.id, ChatModel.clientModeID));
    _blockableOverlayState.applyFfi(gFFI);
    gFFI.imageModel.addCallbackOnFirstImage((String peerId) {
      gFFI.recordingModel
          .updateStatus(bind.sessionGetIsRecording(sessionId: gFFI.sessionId));
      if (gFFI.recordingModel.start) {
        showToast(translate('Automatically record outgoing sessions'));
      }
      _disableAndroidSoftKeyboard(
          isKeyboardVisible: keyboardVisibilityController.isVisible);
    });
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  Future<void> dispose() async {
    WidgetsBinding.instance.removeObserver(this);
    // https://github.com/flutter/flutter/issues/64935
    super.dispose();
    gFFI.dialogManager.hideMobileActionsOverlay(store: false);
    gFFI.inputModel.listenToMouse(false);
    gFFI.imageModel.disposeImage();
    gFFI.cursorModel.disposeImages();
    await gFFI.invokeMethod("enable_soft_keyboard", true);
    _mobileFocusNode.dispose();
    _physicalFocusNode.dispose();
    await gFFI.close();
    _timer?.cancel();
    _timerDidChangeMetrics?.cancel();
    gFFI.dialogManager.dismissAll();
    await SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    WakelockManager.disable(_uniqueKey);
    removeSharedStates(widget.id);
    // `on_voice_call_closed` should be called when the connection is ended.
    // The inner logic of `on_voice_call_closed` will check if the voice call is active.
    // Only one client is considered here for now.
    gFFI.chatModel.onVoiceCallClosed("End connetion");
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {}

  @override
  void didChangeMetrics() {
    // If the soft keyboard is visible and the canvas has been changed(panned or scaled)
    // Don't try reset the view style and focus the cursor.
    if (gFFI.cursorModel.lastKeyboardIsVisible &&
        gFFI.canvasModel.isMobileCanvasChanged) {
      return;
    }

    final newBottom = MediaQueryData.fromView(ui.window).viewInsets.bottom;
    _timerDidChangeMetrics?.cancel();
    _timerDidChangeMetrics = Timer(Duration(milliseconds: 100), () async {
      // We need this comparation because poping up the floating action will also trigger `didChangeMetrics()`.
      if (newBottom != _viewInsetsBottom) {
        gFFI.canvasModel.mobileFocusCanvasCursor();
        _viewInsetsBottom = newBottom;
      }
    });
  }

  // to-do: It should be better to use transparent color instead of the bgColor.
  // But for now, the transparent color will cause the canvas to be white.
  // I'm sure that the white color is caused by the Overlay widget in BlockableOverlay.
  // But I don't know why and how to fix it.
  Widget emptyOverlay(Color bgColor) => BlockableOverlay(
        /// the Overlay key will be set with _blockableOverlayState in BlockableOverlay
        /// see override build() in [BlockableOverlay]
        state: _blockableOverlayState,
        underlying: Container(
          color: bgColor,
        ),
      );

  Widget _bottomWidget() => (_showBar && gFFI.ffiModel.pi.displays.isNotEmpty
      ? getBottomAppBar()
      : Offstage());

  @override
  Widget build(BuildContext context) {
    final keyboardIsVisible =
        keyboardVisibilityController.isVisible && _showEdit;
    final showActionButton = !_showBar || keyboardIsVisible || _showGestureHelp;

    return WillPopScope(
      onWillPop: () async {
        clientClose(sessionId, gFFI);
        return false;
      },
      child: Scaffold(
          // workaround for https://github.com/rustdesk/rustdesk/issues/3131
          floatingActionButtonLocation: keyboardIsVisible
              ? FABLocation(FloatingActionButtonLocation.endFloat, 0, -35)
              : null,
          floatingActionButton: !showActionButton
              ? null
              : FloatingActionButton(
                  mini: !keyboardIsVisible,
                  child: Icon(
                    (keyboardIsVisible || _showGestureHelp)
                        ? Icons.expand_more
                        : Icons.expand_less,
                    color: Colors.white,
                  ),
                  backgroundColor: MyTheme.accent,
                  onPressed: () {
                    setState(() {
                      if (keyboardIsVisible) {
                        _showEdit = false;
                        gFFI.invokeMethod("enable_soft_keyboard", false);
                        _mobileFocusNode.unfocus();
                        _physicalFocusNode.requestFocus();
                      } else if (_showGestureHelp) {
                        _showGestureHelp = false;
                      } else {
                        _showBar = !_showBar;
                      }
                    });
                  }),
          bottomNavigationBar: Obx(() => Stack(
                alignment: Alignment.bottomCenter,
                children: [
                  gFFI.ffiModel.pi.isSet.isTrue &&
                          gFFI.ffiModel.waitForFirstImage.isTrue
                      ? emptyOverlay(MyTheme.canvasColor)
                      : () {
                          gFFI.ffiModel.tryShowAndroidActionsOverlay();
                          return Offstage();
                        }(),
                  _bottomWidget(),
                  gFFI.ffiModel.pi.isSet.isFalse
                      ? emptyOverlay(MyTheme.canvasColor)
                      : Offstage(),
                ],
              )),
          body: Obx(
            () => getRawPointerAndKeyBody(Overlay(
              initialEntries: [
                OverlayEntry(builder: (context) {
                  return Container(
                    color: kColorCanvas,
                    child: SafeArea(
                      child: OrientationBuilder(builder: (ctx, orientation) {
                        if (_currentOrientation != orientation) {
                          Timer(const Duration(milliseconds: 200), () {
                            gFFI.dialogManager
                                .resetMobileActionsOverlay(ffi: gFFI);
                            _currentOrientation = orientation;
                            gFFI.canvasModel.updateViewStyle();
                          });
                        }
                        return Container(
                          color: MyTheme.canvasColor,
                          child: inputModel.isPhysicalMouse.value
                              ? getBodyForMobile()
                              : RawTouchGestureDetectorRegion(
                                  child: getBodyForMobile(),
                                  ffi: gFFI,
                                  isCamera: true,
                                ),
                        );
                      }),
                    ),
                  );
                })
              ],
            )),
          )),
    );
  }

  Widget getBodyForDesktopWithListener() {
    var paints = <Widget>[ImagePaint()];
    return Container(
        color: MyTheme.canvasColor, child: Stack(children: paints));
  }

}
