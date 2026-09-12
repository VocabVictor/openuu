part of 'terminal_page.dart';

extension _TerminalPageBody on _TerminalPageState {
  Widget buildBody() {
    final scaffold = Scaffold(
      resizeToAvoidBottomInset: false, // Disable automatic layout adjustment; manually control UI updates to prevent flickering when the keyboard shows/hides
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: Stack(
        children: [
          Positioned.fill(
            child: SafeArea(
              top: true,
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final heightPx = constraints.maxHeight;
                  return _buildTerminalViewForPlatform(
                    reportMouseInput: isWebDesktop || isAndroid,
                    reportTouchInput: isIOS,
                    terminal: _terminalModel.terminal,
                    controller: _terminalModel.terminalController,
                    textStyle: _getTerminalStyle(),
                    // The following comment is from xterm.dart source code:
                    // Workaround to detect delete key for platforms and IMEs that do not
                    // emit a hardware delete event. Preferred on mobile platforms. [false] by
                    // default.
                    //
                    // Android works fine without this workaround.
                    deleteDetection: isIOS,
                    shortcuts: platformTerminalShortcuts(),
                    onKeyEvent: terminalCopyHandler(
                      _terminalModel.terminal,
                      _terminalModel.terminalController,
                      fallback: _handleTerminalKeyEvent,
                    ),
                    padding: _calculatePadding(heightPx),
                    onSecondaryTapDown: (details, offset) async {
                      final selection = _terminalModel.terminalController.selection;
                      if (selection != null) {
                        final text = _terminalModel.terminal.buffer.getText(selection);
                        _terminalModel.terminalController.clearSelection();
                        await Clipboard.setData(ClipboardData(text: text));
                      } else {
                        await _pasteClipboardText();
                      }
                    },
                  );
                },
              ),
            ),
          ),
          if (_showTerminalExtraKeys) _buildFloatingKeyboard(),
          // iOS-style circular close button in top-right corner
          if (isIOS) _buildCloseButton(),
        ],
      ),
    );

    // Add iOS edge swipe gesture to exit (similar to Android back button)
    if (isIOS) {
      return LayoutBuilder(
        builder: (context, constraints) {
          final screenWidth = constraints.maxWidth;
          // Base thresholds on screen width but clamp to reasonable logical pixel ranges
          // Edge detection region: ~10% of width, clamped between 20 and 80 logical pixels
          final edgeThreshold = (screenWidth * 0.1).clamp(20.0, 80.0);
          // Required horizontal movement: ~25% of width, clamped between 80 and 300 logical pixels
          final swipeThreshold = (screenWidth * 0.25).clamp(80.0, 300.0);

          return RawGestureDetector(
            behavior: HitTestBehavior.translucent,
            gestures: <Type, GestureRecognizerFactory>{
              HorizontalDragGestureRecognizer: GestureRecognizerFactoryWithHandlers<HorizontalDragGestureRecognizer>(
                () => HorizontalDragGestureRecognizer(
                  debugOwner: this,
                  // Only respond to touch input, exclude mouse/trackpad
                  supportedDevices: kTouchBasedDeviceKinds,
                ),
                (HorizontalDragGestureRecognizer instance) {
                  instance
                    // Capture initial touch-down position (before touch slop)
                    ..onDown = (details) {
                      _swipeStartX = details.localPosition.dx;
                      _swipeCurrentX = details.localPosition.dx;
                    }
                    ..onUpdate = (details) {
                      _swipeCurrentX = details.localPosition.dx;
                    }
                    ..onEnd = (details) {
                      // Check if swipe started from left edge and moved right
                      if (_swipeStartX < edgeThreshold && (_swipeCurrentX - _swipeStartX) > swipeThreshold) {
                        clientClose(sessionId, _ffi);
                      }
                      _swipeStartX = 0;
                      _swipeCurrentX = 0;
                    }
                    ..onCancel = () {
                      _swipeStartX = 0;
                      _swipeCurrentX = 0;
                    };
                },
              ),
            },
            child: scaffold,
          );
        },
      );
    }

    return scaffold;
  }

  Widget _buildCloseButton() {
    return Positioned(
      top: 0,
      right: 0,
      child: SafeArea(
        minimum: const EdgeInsets.only(
          top: 16, // iOS standard margin
          right: 16, // iOS standard margin
        ),
        child: Semantics(
          button: true,
          label: translate('Close'),
          child: Container(
            width: 44, // iOS standard tap target size
            height: 44,
            decoration: BoxDecoration(
              color: Colors.black.withOpacity(0.5), // Half transparency
              shape: BoxShape.circle,
            ),
            child: Material(
              color: Colors.transparent,
              shape: const CircleBorder(),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                customBorder: const CircleBorder(),
                onTap: () {
                  clientClose(sessionId, _ffi);
                },
                child: Tooltip(
                  message: translate('Close'),
                  child: const Icon(
                    Icons.chevron_left, // iOS-style back arrow
                    color: Colors.white,
                    size: 28,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

extension _TerminalPageInput on _TerminalPageState {
  EdgeInsets _calculatePadding(double heightPx) {
    if (_cellHeight == null) {
      return const EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0);
    }
    final realHeight = heightPx - _sysKeyboardHeight - _keyboardHeight;
    final rows = (realHeight / _cellHeight!).floor();
    final extraSpace = realHeight - rows * _cellHeight!;
    final topBottom = max(0.0, extraSpace / 2.0);
    return EdgeInsets.only(left: 5.0, right: 5.0, top: topBottom, bottom: topBottom + _sysKeyboardHeight + _keyboardHeight);
  }

  /// Pastes clipboard text through TerminalModel so keyboard-only modifiers and
  /// mobile Enter normalization never alter clipboard data.
  Future<void> _pasteClipboardText() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text;
    if (text == null || !mounted) return;

    await _terminalModel.pasteText(text);
    if (mounted) {
      _terminalModel.terminalController.clearSelection();
    }
  }

  KeyEventResult _handleTerminalKeyEvent(FocusNode _, KeyEvent event) {
    final hardwareKeyboard = HardwareKeyboard.instance;
    final shouldPaste = shouldHandleTerminalPasteShortcut(
      platform: defaultTargetPlatform,
      logicalKey: event.logicalKey,
      isKeyDown: event is KeyDownEvent,
      isKeyRepeat: event is KeyRepeatEvent,
      controlPressed: hardwareKeyboard.isControlPressed,
      metaPressed: hardwareKeyboard.isMetaPressed,
      altPressed: hardwareKeyboard.isAltPressed,
      shiftPressed: hardwareKeyboard.isShiftPressed,
      modifierLockActive: _ctrlLocked || _altLocked,
    );
    if (!shouldPaste) return KeyEventResult.ignored;

    // Only locked virtual modifiers need interception. Without a lock, keep
    // xterm's default hardware paste behavior, including bracketed paste mode.
    unawaited(_pasteClipboardText());
    return KeyEventResult.handled;
  }
}
