part of 'remote_toolbar.dart';

enum _ToolbarEdge { top, right, bottom, left }

_ToolbarEdge _parseToolbarEdge(String? s) {
  switch (s) {
    case 'right':
      return _ToolbarEdge.right;
    case 'bottom':
      return _ToolbarEdge.bottom;
    case 'left':
      return _ToolbarEdge.left;
    default:
      return _ToolbarEdge.top;
  }
}

String _toolbarEdgeToString(_ToolbarEdge e) {
  switch (e) {
    case _ToolbarEdge.top:
      return 'top';
    case _ToolbarEdge.right:
      return 'right';
    case _ToolbarEdge.bottom:
      return 'bottom';
    case _ToolbarEdge.left:
      return 'left';
  }
}

bool _isHorizontalEdge(_ToolbarEdge e) =>
    e == _ToolbarEdge.top || e == _ToolbarEdge.bottom;

const _legacyRemoteMenubarDragX = 'remote-menubar-drag-x';

double _clampToolbarFraction(double fraction, double left, double right) {
  if (fraction < left) fraction = left;
  if (fraction > right) fraction = right;
  return fraction;
}

Size _toolbarSizeForEdge(_ToolbarEdge edge, Size? measured) {
  final isHorizontal = _isHorizontalEdge(edge);
  final fallback = isHorizontal ? const Size(360, 40) : const Size(40, 360);
  final size = measured ?? fallback;
  final long = size.longestSide;
  final short = size.shortestSide;
  return Size(isHorizontal ? long : short, isHorizontal ? short : long);
}

Offset _toolbarOffsetForEdge({
  required _ToolbarEdge edge,
  required double fraction,
  required Size parentSize,
  required Size toolbarSize,
}) {
  final xTravel = parentSize.width - toolbarSize.width;
  final yTravel = parentSize.height - toolbarSize.height;
  switch (edge) {
    case _ToolbarEdge.top:
      return Offset(xTravel * fraction, 0);
    case _ToolbarEdge.bottom:
      return Offset(xTravel * fraction, yTravel);
    case _ToolbarEdge.left:
      return Offset(0, yTravel * fraction);
    case _ToolbarEdge.right:
      return Offset(xTravel, yTravel * fraction);
  }
}

double _fractionForAlignedDrag({
  required double cursor,
  required double grabOffset,
  required double parentExtent,
  required double toolbarExtent,
  required double left,
  required double right,
}) {
  final travelExtent = parentExtent - toolbarExtent;
  if (travelExtent <= 0) {
    return _clampToolbarFraction(0.5, left, right);
  }
  return _clampToolbarFraction(
      (cursor - grabOffset) / travelExtent, left, right);
}

({double left, double right}) _fractionBoundsForEdge(
  _ToolbarEdge edge,
  double left,
  double right,
) {
  return _isHorizontalEdge(edge)
      ? (left: left, right: right)
      : (left: 0, right: 1);
}

String _toolbarRawFraction({
  required bool multiEdgeEnabled,
  required _ToolbarEdge edge,
  required String? savedFraction,
  required String? legacyFraction,
}) {
  if (!multiEdgeEnabled) {
    return (legacyFraction != null && legacyFraction.isNotEmpty)
        ? legacyFraction
        : '0.5';
  }
  if (savedFraction != null && savedFraction.isNotEmpty) {
    return savedFraction;
  }
  if (edge == _ToolbarEdge.top &&
      legacyFraction != null &&
      legacyFraction.isNotEmpty) {
    return legacyFraction;
  }
  return '0.5';
}

// Returns the alignment for the wrapper Align that positions the entire
// toolbar against the given edge at the given fraction along that edge.
// Alignment uses [-1, 1] coordinates (0 = center).
Alignment _alignmentForEdge(_ToolbarEdge edge, double fraction) {
  final f = fraction * 2 - 1;
  switch (edge) {
    case _ToolbarEdge.top:
      return Alignment(f, -1);
    case _ToolbarEdge.bottom:
      return Alignment(f, 1);
    case _ToolbarEdge.left:
      return Alignment(-1, f);
    case _ToolbarEdge.right:
      return Alignment(1, f);
  }
}

// The drag handle hangs off the side of the toolbar facing away from the
// docked edge, so the icons themselves sit flush against that edge.
BorderRadius _collapseHandleBorderRadius(_ToolbarEdge edge) {
  const r = Radius.circular(UiSession.toolbarButtonRadius);
  switch (edge) {
    case _ToolbarEdge.top:
      return const BorderRadius.vertical(bottom: r);
    case _ToolbarEdge.bottom:
      return const BorderRadius.vertical(top: r);
    case _ToolbarEdge.left:
      return const BorderRadius.horizontal(right: r);
    case _ToolbarEdge.right:
      return const BorderRadius.horizontal(left: r);
  }
}

int _monitorMenuQuarterTurns(_ToolbarEdge edge) {
  switch (edge) {
    case _ToolbarEdge.left:
      return 1;
    case _ToolbarEdge.right:
      return 3;
    case _ToolbarEdge.top:
    case _ToolbarEdge.bottom:
      return 0;
  }
}

IconData _toolbarCollapseIcon(_ToolbarEdge edge, bool isCollapsed) {
  switch (edge) {
    case _ToolbarEdge.top:
      return isCollapsed ? Icons.expand_more : Icons.expand_less;
    case _ToolbarEdge.bottom:
      return isCollapsed ? Icons.expand_less : Icons.expand_more;
    case _ToolbarEdge.left:
      return isCollapsed ? Icons.chevron_right : Icons.chevron_left;
    case _ToolbarEdge.right:
      return isCollapsed ? Icons.chevron_left : Icons.chevron_right;
  }
}

class _ToolbarDockingOptions {
  _ToolbarDockingOptions({
    required this.edge,
    required this.fraction,
    required this.multiEdgeEnabled,
  });

  _ToolbarEdge edge;
  double fraction;
  bool multiEdgeEnabled;
}

final _toolbarDockingOptionsBySession = <String, _ToolbarDockingOptions>{};

String _toolbarDockingCacheKey(SessionID sessionId) => sessionId.toString();

_ToolbarDockingOptions? _cachedToolbarDockingOptions(SessionID sessionId) =>
    _toolbarDockingOptionsBySession[_toolbarDockingCacheKey(sessionId)];

void _cacheToolbarDockingOptions({
  required SessionID sessionId,
  required _ToolbarEdge edge,
  required double fraction,
  required bool multiEdgeEnabled,
}) {
  final key = _toolbarDockingCacheKey(sessionId);
  final cached = _toolbarDockingOptionsBySession[key];
  if (cached == null) {
    _toolbarDockingOptionsBySession[key] = _ToolbarDockingOptions(
      edge: edge,
      fraction: fraction,
      multiEdgeEnabled: multiEdgeEnabled,
    );
    return;
  }
  cached.edge = edge;
  cached.fraction = fraction;
  cached.multiEdgeEnabled = multiEdgeEnabled;
}
