import 'dart:async';

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/setting_widgets.dart';
import 'package:flutter_hbb/consts.dart';

import '../../../common.dart';
import '../../../models/model.dart';
import '../../../models/platform_model.dart';
import 'package:flutter_hbb/models/input_model.dart';

customImageQualityDialog(SessionID sessionId, String id, FFI ffi) async {
  double initQuality = kDefaultQuality;
  double initFps = kDefaultFps;
  bool qualitySet = false;
  bool fpsSet = false;

  bool? direct;
  try {
    direct =
        ConnectionTypeState.find(id).direct.value == ConnectionType.strDirect;
  } catch (_) {}
  bool hideFps = (await bind.mainIsUsingPublicServer() && direct != true) ||
      versionCmp(ffi.ffiModel.pi.version, '1.2.0') < 0;
  bool hideMoreQuality =
      (await bind.mainIsUsingPublicServer() && direct != true) ||
          versionCmp(ffi.ffiModel.pi.version, '1.2.2') < 0;

  setCustomValues({double? quality, double? fps}) async {
    debugPrint("setCustomValues quality:$quality, fps:$fps");
    if (quality != null) {
      qualitySet = true;
      await bind.sessionSetCustomImageQuality(
          sessionId: sessionId, value: quality.toInt());
    }
    if (fps != null) {
      fpsSet = true;
      await bind.sessionSetCustomFps(sessionId: sessionId, fps: fps.toInt());
    }
    if (!qualitySet) {
      qualitySet = true;
      await bind.sessionSetCustomImageQuality(
          sessionId: sessionId, value: initQuality.toInt());
    }
    if (!hideFps && !fpsSet) {
      fpsSet = true;
      await bind.sessionSetCustomFps(
          sessionId: sessionId, fps: initFps.toInt());
    }
  }

  final btnClose = dialogButton('Close', onPressed: () async {
    await setCustomValues();
    ffi.dialogManager.dismissAll();
  });

  // quality
  final quality = await bind.sessionGetCustomImageQuality(sessionId: sessionId);
  initQuality = quality != null && quality.isNotEmpty
      ? quality[0].toDouble()
      : kDefaultQuality;
  if (initQuality < kMinQuality ||
      initQuality > (!hideMoreQuality ? kMaxMoreQuality : kMaxQuality)) {
    initQuality = kDefaultQuality;
  }
  // fps
  final fpsOption =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'custom-fps');
  initFps = fpsOption == null
      ? kDefaultFps
      : double.tryParse(fpsOption) ?? kDefaultFps;
  if (initFps < kMinFps || initFps > kMaxFps) {
    initFps = kDefaultFps;
  }

  final content = customImageQualityWidget(
      initQuality: initQuality,
      initFps: initFps,
      setQuality: (v) => setCustomValues(quality: v),
      setFps: (v) => setCustomValues(fps: v),
      showFps: !hideFps,
      showMoreQuality: !hideMoreQuality);
  msgBoxCommon(ffi.dialogManager, 'Custom Image Quality', content, [btnClose]);
}

int? _validateTrackpadSpeed(String text) {
  final speed = int.tryParse(text);
  if (speed == null || speed < kMinTrackpadSpeed || speed > kMaxTrackpadSpeed) {
    BotToast.showText(
      text:
          '${translate('Invalid format')}: $kMinTrackpadSpeed-$kMaxTrackpadSpeed',
      contentColor: Colors.red,
    );
    return null;
  }
  return speed;
}

Future<void> _saveTrackpadSpeed({
  required SessionID sessionId,
  required FFI ffi,
  required int initSpeed,
  required int speed,
}) async {
  if (speed == initSpeed) {
    return;
  }
  await bind.sessionSetTrackpadSpeed(sessionId: sessionId, value: speed);
  await ffi.inputModel.updateTrackpadSpeed();
}

void _showTrackpadSpeedSaveError(Object error, StackTrace stackTrace) {
  debugPrint('Failed to save trackpad speed: $error');
  debugPrintStack(stackTrace: stackTrace);
  BotToast.showText(
    text: translate('Failed'),
    contentColor: Colors.red,
  );
}

List<Widget> _trackpadSpeedDialogActions({
  required bool isSubmitting,
  required VoidCallback close,
  required VoidCallback submit,
}) {
  return [
    dialogButton(
      'Cancel',
      icon: Icon(Icons.close_rounded),
      onPressed: isSubmitting ? null : close,
      isOutline: true,
    ),
    dialogButton(
      'OK',
      icon: Icon(Icons.done_rounded),
      onPressed: isSubmitting ? null : submit,
    ),
  ];
}

void trackpadSpeedDialog(SessionID sessionId, FFI ffi) {
  final initSpeed = ffi.inputModel.trackpadSpeed;
  final curSpeed = SimpleWrapper(initSpeed);
  var speedText = initSpeed.toString();
  var isSubmitting = false;
  ffi.dialogManager.show((setState, close, context) {
    Future<void> submit([String? submittedText]) async {
      if (isSubmitting) {
        return;
      }
      speedText = submittedText ?? speedText;
      final speed = _validateTrackpadSpeed(speedText);
      if (speed == null) {
        return;
      }
      setState(() => isSubmitting = true);
      try {
        await _saveTrackpadSpeed(
          sessionId: sessionId,
          ffi: ffi,
          initSpeed: initSpeed,
          speed: speed,
        );
        close();
      } catch (error, stackTrace) {
        _showTrackpadSpeedSaveError(error, stackTrace);
        setState(() => isSubmitting = false);
      }
    }

    return CustomAlertDialog(
      title: Text(
        translate('Trackpad speed'),
        style: TextStyle(fontSize: 21),
      ),
      content: TrackpadSpeedWidget(
        value: curSpeed,
        onTextChanged: (text) => speedText = text,
        onTextSubmitted: submit,
      ),
      actions: _trackpadSpeedDialogActions(
        isSubmitting: isSubmitting,
        close: close,
        submit: submit,
      ),
      onSubmit: isSubmitting ? null : submit,
      onCancel: isSubmitting ? null : close,
    );
  });
}
