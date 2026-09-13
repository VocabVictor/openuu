import 'dart:async';

import 'package:back_button_interceptor/back_button_interceptor.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../consts.dart';
import '../models/model.dart';
import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'globals.dart';
import 'msgbox_parts.dart';
import 'my_theme.dart';
import 'widgets_misc.dart';

typedef DialogBuilder = CustomAlertDialog Function(
    StateSetter setState, void Function([dynamic]) close, BuildContext context);

class Dialog<T> {
  OverlayEntry? entry;
  Completer<T?> completer = Completer<T?>();

  Dialog();

  void complete(T? res) {
    try {
      if (!completer.isCompleted) {
        completer.complete(res);
      }
    } catch (e) {
      debugPrint("Dialog complete catch error: $e");
    } finally {
      entry?.remove();
    }
  }
}

class OverlayKeyState {
  final _overlayKey = GlobalKey<OverlayState>();

  /// use global overlay by default
  OverlayState? get state =>
      _overlayKey.currentState ?? globalKey.currentState?.overlay;

  GlobalKey<OverlayState>? get key => _overlayKey;
}

class OverlayDialogManager {
  final Map<String, Dialog> _dialogs = {};
  var _overlayKeyState = OverlayKeyState();
  int _tagCount = 0;

  OverlayEntry? _mobileActionsOverlayEntry;
  RxBool mobileActionsOverlayVisible = true.obs;

  setMobileActionsOverlayVisible(bool v, {store = true}) {
    if (store) {
      bind.setLocalFlutterOption(k: kOptionShowMobileAction, v: v ? 'Y' : 'N');
    }
    // No need to read the value from local storage after setting it.
    // It better to toggle the value directly.
    mobileActionsOverlayVisible.value = v;
  }

  loadMobileActionsOverlayVisible() {
    mobileActionsOverlayVisible.value =
        bind.getLocalFlutterOption(k: kOptionShowMobileAction) != 'N';
  }

  void setOverlayState(OverlayKeyState overlayKeyState) {
    _overlayKeyState = overlayKeyState;
  }

  void dismissAll() {
    _dialogs.forEach((key, value) {
      value.complete(null);
      BackButtonInterceptor.removeByName(key);
    });
    _dialogs.clear();
  }

  void dismissByTag(String tag) {
    _dialogs[tag]?.complete(null);
    _dialogs.remove(tag);
    BackButtonInterceptor.removeByName(tag);
  }

  Future<T?> show<T>(DialogBuilder builder,
      {bool clickMaskDismiss = false,
      bool backDismiss = false,
      String? tag,
      bool useAnimation = true,
      bool forceGlobal = false}) {
    final overlayState =
        forceGlobal ? globalKey.currentState?.overlay : _overlayKeyState.state;

    if (overlayState == null) {
      return Future.error(
          "[OverlayDialogManager] Failed to show dialog, _overlayState is null, call [setOverlayState] first");
    }

    final String dialogTag;
    if (tag != null) {
      dialogTag = tag;
    } else {
      dialogTag = _tagCount.toString();
      _tagCount++;
    }

    final dialog = Dialog<T>();
    _dialogs[dialogTag] = dialog;

    close([res]) {
      _dialogs.remove(dialogTag);
      try {
        dialog.complete(res);
      } catch (e) {
        debugPrint("Dialog complete catch error: $e");
      }
      BackButtonInterceptor.removeByName(dialogTag);
    }

    dialog.entry = OverlayEntry(builder: (context) {
      bool innerClicked = false;
      return Listener(
          onPointerUp: (_) {
            if (!innerClicked && clickMaskDismiss) {
              close();
            }
            innerClicked = false;
          },
          child: Container(
              color: Theme.of(context).brightness == Brightness.light
                  ? Colors.black12
                  : Colors.black45,
              child: StatefulBuilder(builder: (context, setState) {
                return Listener(
                  onPointerUp: (_) => innerClicked = true,
                  child: builder(setState, close, overlayState.context),
                );
              })));
    });
    overlayState.insert(dialog.entry!);
    BackButtonInterceptor.add((stopDefaultButtonEvent, routeInfo) {
      if (backDismiss) {
        close();
      }
      return true;
    }, name: dialogTag);
    return dialog.completer.future;
  }

  String showLoading(String text,
      {bool clickMaskDismiss = false,
      bool showCancel = true,
      VoidCallback? onCancel,
      String? tag}) {
    if (tag == null) {
      tag = _tagCount.toString();
      _tagCount++;
    }
    show((setState, close, context) {
      cancel() {
        dismissAll();
        if (onCancel != null) {
          onCancel();
        }
      }

      return CustomAlertDialog(
        content: Container(
            constraints: const BoxConstraints(maxWidth: 240),
            child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const SizedBox(height: 30),
                  const Center(child: CircularProgressIndicator()),
                  const SizedBox(height: 20),
                  Center(
                      child: Text(translate(text),
                          style: const TextStyle(fontSize: 15))),
                  const SizedBox(height: 20),
                  Offstage(
                      offstage: !showCancel,
                      child: Center(
                          child: isDesktop ? dialogButton('Cancel', onPressed: cancel)
                              : TextButton(
                                  style: flatButtonStyle,
                                  onPressed: cancel,
                                  child: Text(translate('Cancel'),
                                      style: const TextStyle(
                                          color: MyTheme.accent)))))
                ])),
        onCancel: showCancel ? cancel : null,
      );
    }, tag: tag);
    return tag;
  }

  void resetMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry == null) return;
    hideMobileActionsOverlay();
    showMobileActionsOverlay(ffi: ffi);
  }

  void showMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry != null) return;
    final overlayState = _overlayKeyState.state;
    if (overlayState == null) return;

    final overlay = makeMobileActionsOverlayEntry(
      () => hideMobileActionsOverlay(),
      ffi: ffi,
    );
    overlayState.insert(overlay);
    _mobileActionsOverlayEntry = overlay;
    setMobileActionsOverlayVisible(true);
  }

  void hideMobileActionsOverlay({store = true}) {
    if (_mobileActionsOverlayEntry != null) {
      _mobileActionsOverlayEntry!.remove();
      _mobileActionsOverlayEntry = null;
      setMobileActionsOverlayVisible(false, store: store);
      return;
    }
  }

  void toggleMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry == null) {
      showMobileActionsOverlay(ffi: ffi);
    } else {
      hideMobileActionsOverlay();
    }
  }

  bool existing(String tag) {
    return _dialogs.keys.contains(tag);
  }
}
