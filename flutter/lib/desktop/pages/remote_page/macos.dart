part of 'remote_page.dart';

extension _RemotePageMacOS on _RemotePageState {
  Future<void> _normalizeWaylandKeyboardModeIfNeeded() async {
    if (!mounted ||
        _waylandKeyboardModeNormalized ||
        _waylandKeyboardModeNormalizing) {
      return;
    }
    _waylandKeyboardModeNormalizing = true;
    try {
      final pi = _ffi.ffiModel.pi;
      if (pi.platform != kPeerPlatformLinux || !pi.isWayland) return;
      final mapSupported = bind.sessionIsKeyboardModeSupported(
          sessionId: sessionId, mode: kKeyMapMode);
      if (!mapSupported) return;
      final current = await bind.sessionGetKeyboardMode(sessionId: sessionId);
      if (!mounted) return;
      if (current == kKeyMapMode) {
        _waylandKeyboardModeNormalized = true;
        return;
      }
      await bind.sessionSetKeyboardMode(
          sessionId: sessionId, value: kKeyMapMode);
      if (!mounted) return;
      await _ffi.inputModel.updateKeyboardMode();
      if (!mounted) return;
      _waylandKeyboardModeNormalized = true;
    } catch (e, st) {
      debugPrint('Failed to normalize Wayland keyboard mode: $e');
      debugPrintStack(stackTrace: st);
    } finally {
      _waylandKeyboardModeNormalizing = false;
    }
  }

  /// Cancel the pointer lock center debounce timer
  void _cancelPointerLockCenterDebounceTimer() {
    _pointerLockCenterDebounceTimer?.cancel();
    _pointerLockCenterDebounceTimer = null;
  }

  bool get _isSelectedTab {
    final controller = widget.tabController;
    if (controller == null) return true;
    final tabState = controller.state.value;
    final selected = tabState.selected;
    return selected >= 0 &&
        selected < tabState.tabs.length &&
        tabState.tabs[selected].key == widget.id;
  }

  // Every Windows requestFocus() must pass this, or a blocking dialog or an
  // inactive tab could hand remote input to this page.
  bool get _windowsCanFocusRemoteInput =>
      _isSelectedTab && _blockableOverlayState.middleBlocked.isFalse;

  bool get _isMacOSKeyboardContextActive {
    return stateGlobal.isFocused.value && !_isWindowBlur && _isSelectedTab;
  }

  void _onMacOSTabStateChanged(DesktopTabState _) {
    if (!_isSelectedTab) {
      _macOSFullScreenFocusRecovery.cancel();
      _syncMacOSKeyboardGrab();
      return;
    }
    // Tab listeners run synchronously. Defer the selected page so the previous
    // page releases first; a late leave from it can disable the new session.
    scheduleMicrotask(() {
      if (mounted) {
        _syncMacOSKeyboardGrab(reassert: true);
      }
    });
  }

  void _releaseMacOSRemoteInput() {
    _macOSFullScreenFocusRecovery.cancel();
    _macOSExplicitFocusRequestPending = false;
    _macOSInputSuppressed = true;
    _macOSLocalFocusLost = true;
    _ffi.inputModel.enterOrLeave(false);
    _macOSInputActive = false;
    _rawKeyFocusNode.unfocus();
  }

  void _onMacOSFocusChange() {
    // requestFocus() notifies later; only a recorded explicit request may clear
    // the local-focus-loss latch.
    if (_rawKeyFocusNode.hasPrimaryFocus) {
      final explicitRequest = _macOSExplicitFocusRequestPending;
      _macOSExplicitFocusRequestPending = false;
      if (explicitRequest && _isMacOSKeyboardContextActive) {
        _macOSLocalFocusLost = false;
      }
      _syncMacOSKeyboardGrab(allowInactiveLifecycle: explicitRequest);
    } else {
      if (_macOSInputActive) {
        _ffi.inputModel.enterOrLeave(false);
        _macOSInputActive = false;
      }
      if (_isMacOSKeyboardContextActive) {
        _macOSLocalFocusLost = true;
      }
    }
  }

  // 1. Sync the keyboard grab state with the current context.
  // 2. Call enterOrLeave() to update the input state in the FFI layer.
  // 3. Request or unfocus the raw key focus node based on the current context.
  // Flutter focus and native input are separate; native input activates only
  // after the FocusNode has primary focus.
  void _syncMacOSKeyboardGrab({
    bool reassert = false,
    bool allowInactiveLifecycle = false,
  }) {
    if (!isMacOS) return;
    // A secondary engine may stay hidden while its window is visible, so
    // explicit pointer/fullscreen recovery must bypass the global lifecycle.
    final lifecycleAllowsInput = allowInactiveLifecycle ||
        _macOSLifecycleState == null ||
        _macOSLifecycleState == AppLifecycleState.resumed;
    // Input stays pointer-gated except for focused fullscreen recovery, which
    // compensates when macOS omits PointerEnter during a Space switch.
    final shouldFocus = lifecycleAllowsInput &&
        _isMacOSKeyboardContextActive &&
        !_macOSInputSuppressed &&
        _blockableOverlayState.middleBlocked.isFalse &&
        _cursorOverImage.value &&
        !_macOSLocalFocusLost;
    final hasFocus = _rawKeyFocusNode.hasPrimaryFocus;
    final shouldActivateInput = shouldFocus && hasFocus;

    if (shouldActivateInput != _macOSInputActive ||
        (shouldActivateInput && reassert)) {
      _ffi.inputModel.enterOrLeave(shouldActivateInput);
    }
    _macOSInputActive = shouldActivateInput;

    if (!shouldFocus) {
      _macOSExplicitFocusRequestPending = false;
      if (hasFocus) _rawKeyFocusNode.unfocus();
    } else if (!hasFocus) {
      _macOSExplicitFocusRequestPending = allowInactiveLifecycle;
      _rawKeyFocusNode.requestFocus();
    } else {
      _macOSExplicitFocusRequestPending = false;
    }
  }

  void _restoreMacOSKeyboardAfterFullScreen({
    required int generation,
    bool allowHiddenLifecycle = false,
  }) {
    // Fullscreen callbacks preserve recovery while hidden. Native window focus
    // may bypass a stale hidden lifecycle for the newly visible Space.
    if (!_macOSFullScreenFocusRecovery.isCurrent(generation) ||
        (!allowHiddenLifecycle &&
            _macOSLifecycleState == AppLifecycleState.hidden)) {
      return;
    }
    final contextActive =
        stateGlobal.isFocused.value && !_isWindowBlur && _isSelectedTab;
    // macOS can focus a fullscreen Space without sending PointerEnter. Native
    // window focus is authoritative here; a later blur cancels this generation
    // before an off-screen window can restore input.
    final shouldInferPointerInside = !_cursorOverImage.value &&
        allowHiddenLifecycle &&
        stateGlobal.fullscreen.isTrue &&
        contextActive;
    final canRestore = contextActive &&
        _blockableOverlayState.middleBlocked.isFalse &&
        (_cursorOverImage.value || shouldInferPointerInside);
    if (!_macOSFullScreenFocusRecovery.consume(generation)) return;
    if (!canRestore) {
      // Consuming recovery here requires a later pointer/window/tab event.
      return;
    }
    if (shouldInferPointerInside) {
      _cursorOverImage.value = true;
    }
    _macOSLocalFocusLost = false;
    stateGlobal.getInputSource(force: true);
    _syncMacOSKeyboardGrab(reassert: true, allowInactiveLifecycle: true);
  }

  void _scheduleMacOSKeyboardAfterFullScreen({
    required int generation,
    bool allowHiddenLifecycle = false,
  }) {
    // Fullscreen can deliver FocusNode loss after its callback; wait for frame
    // completion and then advance one event-loop turn before restoring.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      Timer.run(() {
        if (mounted) {
          _restoreMacOSKeyboardAfterFullScreen(
            generation: generation,
            allowHiddenLifecycle: allowHiddenLifecycle,
          );
        }
      });
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  void _queueMacOSKeyboardAfterFullScreen({
    bool allowHiddenLifecycle = false,
  }) {
    final generation = _macOSFullScreenFocusRecovery.queue();
    if (_macOSLifecycleState == AppLifecycleState.paused ||
        _macOSLifecycleState == AppLifecycleState.detached) {
      _macOSFullScreenFocusRecovery.cancel();
      return;
    }
    _scheduleMacOSKeyboardAfterFullScreen(
      generation: generation,
      allowHiddenLifecycle: allowHiddenLifecycle,
    );
  }
}
