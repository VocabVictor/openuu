part of 'model.dart';

// data for cursor
class CursorData {
  final String peerId;
  final String id;
  final img2.Image image;
  double scale;
  Uint8List? data;
  final double hotxOrigin;
  final double hotyOrigin;
  double hotx;
  double hoty;
  final int width;
  final int height;

  CursorData({
    required this.peerId,
    required this.id,
    required this.image,
    required this.scale,
    required this.data,
    required this.hotxOrigin,
    required this.hotyOrigin,
    required this.width,
    required this.height,
  })  : hotx = hotxOrigin * scale,
        hoty = hotyOrigin * scale;

  int _doubleToInt(double v) => (v * 10e6).round().toInt();

  double _checkUpdateScale(double scale) {
    double oldScale = this.scale;
    if (scale != 1.0) {
      // Update data if scale changed.
      final tgtWidth = (width * scale).toInt();
      final tgtHeight = (width * scale).toInt();
      if (tgtWidth < kMinCursorSize || tgtHeight < kMinCursorSize) {
        double sw = kMinCursorSize.toDouble() / width;
        double sh = kMinCursorSize.toDouble() / height;
        scale = sw < sh ? sh : sw;
      }
    }

    if (_doubleToInt(oldScale) != _doubleToInt(scale)) {
      if (isWindows) {
        data = img2
            .copyResize(
              image,
              width: (width * scale).toInt(),
              height: (height * scale).toInt(),
              interpolation: img2.Interpolation.average,
            )
            .getBytes(order: img2.ChannelOrder.bgra);
      } else {
        data = Uint8List.fromList(
          img2.encodePng(
            img2.copyResize(
              image,
              width: (width * scale).toInt(),
              height: (height * scale).toInt(),
              interpolation: img2.Interpolation.average,
            ),
          ),
        );
      }
    }

    this.scale = scale;
    hotx = hotxOrigin * scale;
    hoty = hotyOrigin * scale;
    return scale;
  }

  String updateGetKey(double scale) {
    scale = _checkUpdateScale(scale);
    return '${peerId}_${id}_${_doubleToInt(width * scale)}_${_doubleToInt(height * scale)}';
  }
}

const _forbiddenCursorPng =
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAAXNSR0IB2cksfwAAAAlwSFlzAAALEwAACxMBAJqcGAAAAkZQTFRFAAAA2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4GWAwCAAAAAAAA2B4GAAAAMTExAAAAAAAA2B4G2B4G2B4GAAAAmZmZkZGRAQEBAAAA2B4G2B4G2B4G////oKCgAwMDag8D2B4G2B4G2B4Gra2tBgYGbg8D2B4G2B4Gubm5CQkJTwsCVgwC2B4GxcXFDg4OAAAAAAAA2B4G2B4Gz8/PFBQUAAAAAAAA2B4G2B4G2B4G2B4G2B4G2B4G2B4GDgIA2NjYGxsbAAAAAAAA2B4GFwMB4eHhIyMjAAAAAAAA2B4G6OjoLCwsAAAAAAAA2B4G2B4G2B4G2B4G2B4GCQEA4ODgv7+/iYmJY2NjAgICAAAA9PT0Ojo6AAAAAAAAAAAA+/v7SkpKhYWFr6+vAAAAAAAA8/PzOTk5ERER9fX1KCgoAAAAgYGBKioqAAAAAAAApqamlpaWAAAAAAAAAAAAAAAAAAAAAAAALi4u/v7+GRkZAAAAAAAAAAAAAAAAAAAAfn5+AAAAAAAAV1dXkJCQAAAAAAAAAQEBAAAAAAAAAAAA7Hz6BAAAAMJ0Uk5TAAIWEwEynNz6//fVkCAatP2fDUHs6cDD8d0mPfT5fiEskiIR584A0gejr3AZ+P4plfALf5ZiTL85a4ziD6697fzN3UYE4v/4TwrNHuT///tdRKZh///+1U/ZBv///yjb///eAVL//50Cocv//6oFBbPvpGZCbfT//7cIhv///8INM///zBEcWYSZmO7//////1P////ts/////8vBv//////gv//R/z///QQz9sevP///2waXhNO/+fc//8mev/5gAe2r90MAAAByUlEQVR4nGNggANGJmYWBpyAlY2dg5OTi5uHF6s0H78AJxRwCAphyguLgKRExcQlQLSkFLq8tAwnp6ycPNABjAqKQKNElVDllVU4OVVhVquJA81Q10BRoAkUUYbJa4Edoo0sr6PLqaePLG/AyWlohKTAmJPTBFnelAFoixmSAnNOTgsUeQZLTk4rJAXWnJw2EHlbiDyDPCenHZICe04HFrh+RydnBgYWPU5uJAWinJwucPNd3dw9GDw5Ob2QFHBzcnrD7ffx9fMPCOTkDEINhmC4+3x8Q0LDwlEDIoKTMzIKKg9SEBIdE8sZh6SAJZ6Tkx0qD1YQkpCYlIwclCng0AXLQxSEpKalZyCryATKZwkhKQjJzsnNQ1KQXwBUUVhUXBJYWgZREFJeUVmFpMKlWg+anmqgCkJq6+obkG1pLEBTENLU3NKKrIKhrb2js8u4G6Kgpze0r3/CRAZMAHbkpJDJU6ZMmTqtFbuC6TNmhsyaMnsOFlmwgrnzpsxfELJwEXZ5Bp/FS3yWLlsesmLlKuwKVk9Ys5Zh3foN0zduwq5g85atDAzbpqSGbN9RhV0FGOzctWH3lD14FOzdt3H/gQw8Cg4u2gQPAwBYDXXdIH+wqAAAAABJRU5ErkJggg==';

const _defaultCursorPng =
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAFmSURBVFiF7dWxSlxREMbx34QFDRowYBchZSxSCWlMCOwD5FGEFHap06UI7KPsAyyEEIQFqxRaCqYTsqCJFsKkuAeRXb17wrqV918dztw55zszc2fo6Oh47MR/e3zO1/iAHWmznHKGQwx9ip/LEbCfazbsoY8j/JLOhcC6sCW9wsjEwJf483AC9nPNc1+lFRwI13d+l3rYFS799rFGxJMqARv2pBXh+72XQ7gWvklPS7TmMl9Ak/M+DqrENvxAv/guKKApuKPWl0/TROK4+LbSqzhuB+OZ3fRSeFPWY+Fkyn56Y29hfgTSpnQ+s98cvorVey66uPlNFxKwZOYLCGfCs5n9NMYVrsp6mvXSoFqpqYFDvMBkStgJJe93dZOwVXxbqUnBENulydSReqUrDhcX0PT2EXarBYS3GNXMhboinBgIl9K71kg0L3+PvyYGdVpruT2MwrF0iotiXfIwus0Dj+OOjo6Of+e7ab74RkpgAAAAAElFTkSuQmCC';

const kPreForbiddenCursorId = "-2";

final preForbiddenCursor = PredefinedCursor(
  png: _forbiddenCursorPng,
  id: kPreForbiddenCursorId,
);

const kPreDefaultCursorId = "-1";

final preDefaultCursor = PredefinedCursor(
  png: _defaultCursorPng,
  id: kPreDefaultCursorId,
  hotxGetter: (double w) => w / 2,
  hotyGetter: (double h) => h / 2,
);

class PredefinedCursor {
  ui.Image? _image;
  img2.Image? _image2;
  CursorData? _cache;
  String png;
  String id;
  double Function(double)? hotxGetter;
  double Function(double)? hotyGetter;

  PredefinedCursor(
      {required this.png, required this.id, this.hotxGetter, this.hotyGetter}) {
    init();
  }

  ui.Image? get image => _image;
  CursorData? get cache => _cache;

  init() {
    _image2 = img2.decodePng(base64Decode(png));
    if (_image2 != null) {
      // The png type of forbidden cursor image is `PngColorType.indexed`.
      if (id == kPreForbiddenCursorId) {
        _image2 = _image2!.convert(format: img2.Format.uint8, numChannels: 4);
      }

      () async {
        final defaultImg = _image2!;
        // This function is called only one time, no need to care about the performance.
        Uint8List data = defaultImg.getBytes(order: img2.ChannelOrder.rgba);
        _image?.dispose();
        _image = await img.decodeImageFromPixels(
            data, defaultImg.width, defaultImg.height, ui.PixelFormat.rgba8888);
        if (_image == null) {
          print("decodeImageFromPixels failed, pre-defined cursor $id");
          return;
        }
        double scale = 1.0;
        if (isWindows) {
          data = _image2!.getBytes(order: img2.ChannelOrder.bgra);
        } else {
          data = Uint8List.fromList(img2.encodePng(_image2!));
        }

        _cache = CursorData(
          peerId: '',
          id: id,
          image: _image2!.clone(),
          scale: scale,
          data: data,
          hotxOrigin:
              hotxGetter != null ? hotxGetter!(_image2!.width.toDouble()) : 0,
          hotyOrigin:
              hotyGetter != null ? hotyGetter!(_image2!.height.toDouble()) : 0,
          width: _image2!.width,
          height: _image2!.height,
        );
      }();
    }
  }
}
