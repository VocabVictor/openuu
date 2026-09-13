import 'dart:ui';

import 'package:flutter_hbb/common/window_fit.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const work = Rect.fromLTWH(0, 0, 1920, 1040);

  test('a 16:9 remote is capped by 90% of the work area height, centred',
      () {
    final r = fitRemoteWindowFrame(const Size(1920, 1080), work);
    expect(r.height, 936);
    expect(r.width, 1664);
    expect(r.center.dx, closeTo(960, 1));
    expect(r.center.dy, closeTo(520, 1));
  });

  test('a small remote opens at its own size, not upscaled', () {
    final r = fitRemoteWindowFrame(const Size(1280, 720), work);
    expect(r.width, 1280);
    expect(r.height, 720);
  });

  test('a tall remote keeps its aspect ratio and stays inside the work area',
      () {
    final r = fitRemoteWindowFrame(const Size(1080, 1920), work);
    expect(r.height, lessThanOrEqualTo(work.height));
    expect(r.width / r.height, closeTo(1080 / 1920, .01));
    expect(r.width, greaterThanOrEqualTo(526));
  });

  test('a remote larger than the screen never exceeds the work area', () {
    final r = fitRemoteWindowFrame(const Size(5120, 2880), work);
    expect(r.width <= work.width, isTrue);
    expect(r.height <= work.height, isTrue);
  });

  test('a tiny remote still opens at the minimum size, centred', () {
    final r = fitRemoteWindowFrame(const Size(320, 240), work);
    expect(r.width, 640);
    expect(r.height, 480);
    expect(r.left, 640);
  });

  test('unknown remote size falls back to 16:9', () {
    final r = fitRemoteWindowFrame(Size.zero, work);
    expect(r.width / r.height, closeTo(16 / 9, .01));
  });

  test('the work area offset is respected', () {
    final r = fitRemoteWindowFrame(
        const Size(1920, 1080), const Rect.fromLTWH(1920, 0, 1920, 1040));
    expect(r.left, greaterThanOrEqualTo(1920));
    expect(r.right, lessThanOrEqualTo(3840));
  });
}
