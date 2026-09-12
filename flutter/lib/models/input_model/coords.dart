part of 'input_model.dart';

class CanvasCoords {
  double x = 0;
  double y = 0;
  double scale = 1.0;
  double scrollX = 0;
  double scrollY = 0;
  ScrollStyle scrollStyle = ScrollStyle.scrollauto;
  Size size = Size.zero;

  CanvasCoords();

  Map<String, dynamic> toJson() {
    return {
      'x': x,
      'y': y,
      'scale': scale,
      'scrollX': scrollX,
      'scrollY': scrollY,
      'scrollStyle': scrollStyle.toJson(),
      'size': {
        'w': size.width,
        'h': size.height,
      }
    };
  }

  static CanvasCoords fromJson(Map<String, dynamic> json) {
    final model = CanvasCoords();
    model.x = json['x'];
    model.y = json['y'];
    model.scale = json['scale'];
    model.scrollX = json['scrollX'];
    model.scrollY = json['scrollY'];
    model.scrollStyle =
        ScrollStyle.fromJson(json['scrollStyle'], ScrollStyle.scrollauto);
    model.size = Size(json['size']['w'], json['size']['h']);
    return model;
  }

  static CanvasCoords fromCanvasModel(CanvasModel model) {
    final coords = CanvasCoords();
    coords.x = model.x;
    coords.y = model.y;
    coords.scale = model.scale;
    coords.scrollX = model.scrollX;
    coords.scrollY = model.scrollY;
    coords.scrollStyle = model.scrollStyle;
    coords.size = model.size;
    return coords;
  }
}

class CursorCoords {
  Offset offset = Offset.zero;

  CursorCoords();

  Map<String, dynamic> toJson() {
    return {
      'offset_x': offset.dx,
      'offset_y': offset.dy,
    };
  }

  static CursorCoords fromJson(Map<String, dynamic> json) {
    final model = CursorCoords();
    model.offset = Offset(json['offset_x'], json['offset_y']);
    return model;
  }

  static CursorCoords fromCursorModel(CursorModel model) {
    final coords = CursorCoords();
    coords.offset = model.offset;
    return coords;
  }
}

class RemoteWindowCoords {
  RemoteWindowCoords(
      this.windowRect, this.canvas, this.cursor, this.remoteRect);
  Rect windowRect;
  CanvasCoords canvas;
  CursorCoords cursor;
  Rect remoteRect;
  Offset relativeOffset = Offset.zero;

  Map<String, dynamic> toJson() {
    return {
      'canvas': canvas.toJson(),
      'cursor': cursor.toJson(),
      'windowRect': rectToJson(windowRect),
      'remoteRect': rectToJson(remoteRect),
    };
  }

  static Map<String, dynamic> rectToJson(Rect r) {
    return {
      'l': r.left,
      't': r.top,
      'w': r.width,
      'h': r.height,
    };
  }

  static Rect rectFromJson(Map<String, dynamic> json) {
    return Rect.fromLTWH(
      json['l'],
      json['t'],
      json['w'],
      json['h'],
    );
  }

  RemoteWindowCoords.fromJson(Map<String, dynamic> json)
      : windowRect = rectFromJson(json['windowRect']),
        canvas = CanvasCoords.fromJson(json['canvas']),
        cursor = CursorCoords.fromJson(json['cursor']),
        remoteRect = rectFromJson(json['remoteRect']);
}

extension ToString on MouseButtons {
  String get value {
    switch (this) {
      case MouseButtons.left:
        return 'left';
      case MouseButtons.right:
        return 'right';
      case MouseButtons.wheel:
        return 'wheel';
      case MouseButtons.back:
        return 'back';
      case MouseButtons.forward:
        return 'forward';
    }
  }
}
