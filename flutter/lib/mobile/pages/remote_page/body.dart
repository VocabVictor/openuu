part of 'remote_page.dart';

extension _RemotePageBody on _RemotePageState {
  Widget getRawPointerAndKeyBody(Widget child) {
    final ffiModel = Provider.of<FfiModel>(context);
    return RawPointerMouseRegion(
      cursor: ffiModel.keyboard ? SystemMouseCursors.none : MouseCursor.defer,
      inputModel: inputModel,
      // Disable RawKeyFocusScope before the connecting is established.
      // The "Delete" key on the soft keyboard may be grabbed when inputting the password dialog.
      child: gFFI.ffiModel.pi.isSet.isTrue
          ? RawKeyFocusScope(
              focusNode: _physicalFocusNode,
              inputModel: inputModel,
              child: child)
          : child,
    );
  }

  Widget getBottomAppBar() {
    final ffiModel = Provider.of<FfiModel>(context);
    return BottomAppBar(
      elevation: 10,
      color: MyTheme.accent,
      child: Row(
        mainAxisSize: MainAxisSize.max,
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: <Widget>[
          Row(
              children: <Widget>[
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.clear),
                      onPressed: () {
                        clientClose(sessionId, gFFI);
                      },
                    ),
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.tv),
                      onPressed: () {
                        _setState(() => _showEdit = false);
                        showOptions(context, widget.id, gFFI.dialogManager);
                      },
                    )
                  ] +
                  (ffiModel.viewOnly || !ffiModel.keyboard
                      ? []
                      : gFFI.ffiModel.isPeerAndroid
                          ? [
                              IconButton(
                                  color: Colors.white,
                                  icon: Icon(Icons.keyboard),
                                  onPressed: openKeyboard),
                              IconButton(
                                color: Colors.white,
                                icon: const Icon(Icons.build),
                                onPressed: () => gFFI.dialogManager
                                    .toggleMobileActionsOverlay(ffi: gFFI),
                              )
                            ]
                          : [
                              IconButton(
                                  color: Colors.white,
                                  icon: Icon(Icons.keyboard),
                                  onPressed: openKeyboard),
                              IconButton(
                                color: Colors.white,
                                icon: Icon(gFFI.ffiModel.touchMode
                                    ? Icons.touch_app
                                    : Icons.mouse),
                                onPressed: () => _setState(
                                    () => _showGestureHelp = !_showGestureHelp),
                              ),
                            ]) +
                  (<Widget>[
                          futureBuilder(
                              future: gFFI.invokeMethod(
                                  "get_value", "KEY_IS_SUPPORT_VOICE_CALL"),
                              hasData: (isSupportVoiceCall) => IconButton(
                                    color: Colors.white,
                                    icon: isAndroid && isSupportVoiceCall
                                        ? SvgPicture.asset('assets/chat.svg',
                                            colorFilter: ColorFilter.mode(
                                                Colors.white, BlendMode.srcIn))
                                        : Icon(Icons.message),
                                    onPressed: () =>
                                        isAndroid && isSupportVoiceCall
                                            ? showChatOptions(widget.id)
                                            : onPressedTextChat(widget.id),
                                  ))
                        ]) +
                  [
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.more_vert),
                      onPressed: () {
                        _setState(() => _showEdit = false);
                        showActions(widget.id);
                      },
                    ),
                  ]),
          Obx(() => IconButton(
                color: Colors.white,
                icon: Icon(Icons.expand_more),
                onPressed: gFFI.ffiModel.waitForFirstImage.isTrue
                    ? null
                    : () {
                        _setState(() => _showBar = !_showBar);
                      },
              )),
        ],
      ),
    );
  }

  bool get showCursorPaint =>
      !gFFI.ffiModel.isPeerAndroid &&
      !gFFI.canvasModel.cursorEmbedded &&
      !gFFI.inputModel.relativeMouseMode.value;

  Widget getBodyForMobile() {
    final keyboardIsVisible = keyboardVisibilityController.isVisible;
    return Container(
        color: MyTheme.canvasColor,
        child: Stack(children: () {
          final paints = [
            ImagePaint(ffiModel: gFFI.ffiModel),
            Positioned(
              top: 10,
              right: 10,
              child: QualityMonitor(gFFI.qualityMonitorModel),
            ),
            KeyHelpTools(
                keyboardIsVisible: keyboardIsVisible,
                showGestureHelp: _showGestureHelp),
            SizedBox(
              width: 0,
              height: 0,
              child: !_showEdit
                  ? Container()
                  : TextFormField(
                      textInputAction: TextInputAction.newline,
                      autocorrect: false,
                      // Flutter 3.16.9 Android.
                      // `enableSuggestions` causes secure keyboard to be shown.
                      // https://github.com/flutter/flutter/issues/139143
                      // https://github.com/flutter/flutter/issues/146540
                      // enableSuggestions: false,
                      autofocus: true,
                      focusNode: _mobileFocusNode,
                      maxLines: null,
                      controller: _textController,
                      // trick way to make backspace work always
                      keyboardType: TextInputType.multiline,
                      // `onChanged` may be called depending on the input method if this widget is wrapped in
                      // `Focus(onKeyEvent: ..., child: ...)`
                      // For `Backspace` button in the soft keyboard:
                      // en/fr input method:
                      //      1. The button will not trigger `onKeyEvent` if the text field is not empty.
                      //      2. The button will trigger `onKeyEvent` if the text field is empty.
                      // ko/zh/ja input method: the button will trigger `onKeyEvent`
                      //                     and the event will not popup if `KeyEventResult.handled` is returned.
                      onChanged: handleSoftKeyboardInput,
                    ).workaroundFreezeLinuxMint(),
            ),
          ];
          if (showCursorPaint) {
            paints.add(CursorPaint(widget.id));
          }
          if (gFFI.ffiModel.touchMode) {
            paints.add(FloatingMouse(
              ffi: gFFI,
            ));
          } else {
            paints.add(FloatingMouseWidgets(
              ffi: gFFI,
            ));
          }
          return paints;
        }()));
  }

}
