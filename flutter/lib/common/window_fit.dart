import 'dart:math' as math;
import 'dart:ui';

/// The frame a remote-desktop window opens with when nothing is remembered
/// for the peer: the remote at its own size, shrunk with its aspect ratio
/// kept to fit [maxFraction] of the work area, never below [minSize], centred
/// in the work area. An unknown remote size counts as 16:9.
Rect fitRemoteWindowFrame(Size remote, Rect workArea,
    {double maxFraction = .9, Size minSize = const Size(640, 400)}) {
  final maxW = workArea.width * maxFraction;
  final maxH = workArea.height * maxFraction;
  final known = remote.width > 0 && remote.height > 0;
  final aspect = known ? remote.width / remote.height : 16 / 9;
  var w = known ? math.min(remote.width, maxW) : maxW;
  var h = w / aspect;
  if (h > maxH) {
    h = maxH;
    w = h * aspect;
  }
  // The minimum size may push past the 90% cap but never past the work area,
  // so the aspect ratio survives.
  var up = math.max(math.max(minSize.width / w, minSize.height / h), 1.0);
  up = math.min(up, math.min(workArea.width / w, workArea.height / h));
  w *= up;
  h *= up;
  return Rect.fromLTWH(
      workArea.left + (workArea.width - w) / 2,
      workArea.top + (workArea.height - h) / 2,
      w.roundToDouble(),
      h.roundToDouble());
}

/// window_size reports screen frames in physical pixels on Windows while
/// window frames are set in logical pixels; divide by the screen's scale.
Rect logicalWorkArea(Rect physicalVisibleFrame, double scaleFactor) {
  final scale = scaleFactor > 0 ? scaleFactor : 1.0;
  return Rect.fromLTWH(
      physicalVisibleFrame.left / scale,
      physicalVisibleFrame.top / scale,
      physicalVisibleFrame.width / scale,
      physicalVisibleFrame.height / scale);
}

/// A session window's frame is remembered for its peer only after the user
/// resized or moved the window, at least 3 s after the first-open fit;
/// otherwise every peer would remember the default frame forever.
bool sessionWindowUserSized = false;
DateTime? sessionWindowFittedAt;

void noteSessionWindowFrameEvent() {
  final at = sessionWindowFittedAt;
  if (at != null && DateTime.now().difference(at) > const Duration(seconds: 3)) {
    sessionWindowUserSized = true;
  }
}
