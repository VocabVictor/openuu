part of 'floating_mouse.dart';

extension _FloatingMouseBuild on _FloatingMouseState {
  Widget _buildMouseWithHide() {
    double minMouseScale = (_baseMouseScale * 0.3);
    if (!_isExpanded) {
      return SizedBox(
          width: mouseWidth,
          height: mouseHeight,
          child: GestureDetector(
            onPanUpdate: _onDragHandleUpdate,
            onTap: () {
              _setState(() {
                _mouseScale = _baseMouseScale;
                _isExpanded = true;
                _position -= _expandOffset;
              });
              _resetCollapseTimer();
            },
            child: MouseBody(
              scrollWheelUpKey: _scrollWheelUpKey,
              scrollWheelDownKey: _scrollWheelDownKey,
              mouseWidgetKey: _mouseWidgetKey,
              inputModel: _isExpanded ? _inputModel : null,
              scale: _mouseScale,
              resetCollapseTimer: _resetCollapseTimer,
            ),
          ));
    } else {
      return SizedBox(
        width: mouseWidth,
        height: mouseHeight,
        child: Column(
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                CursorPaint(
                  key: _cursorPaintKey,
                  scale: _mouseScale,
                ),
                const Spacer(),
                GestureDetector(
                  onTap: () {
                    _collapseTimer?.cancel();
                    _setState(() {
                      _mouseScale = minMouseScale;
                      _isExpanded = false;
                      _position += _expandOffset;
                    });
                  },
                  child: Container(
                    width: 18 * _mouseScale,
                    height: 18 * _mouseScale,
                    child: Center(
                      child: Container(
                        width: 14 * _mouseScale,
                        height: 14 * _mouseScale,
                        decoration: const BoxDecoration(
                          color: Colors.grey,
                          shape: BoxShape.circle,
                        ),
                        alignment: Alignment.center,
                        child: Icon(Icons.close,
                            color: Colors.white, size: 12 * _mouseScale),
                      ),
                    ),
                  ),
                ),
              ],
            ),
            Padding(
                padding: EdgeInsets.only(left: 14 * _mouseScale),
                child: MouseBody(
                  scrollWheelUpKey: _scrollWheelUpKey,
                  scrollWheelDownKey: _scrollWheelDownKey,
                  mouseWidgetKey: _mouseWidgetKey,
                  onPointerMoveUpdate: _onBodyPointerMoveUpdate,
                  cancelCanvasScroll: _canvasScrollState.tryCancel,
                  setCanvasScrollPressed: _canvasScrollState.setPressedSpeed,
                  setCanvasScrollReleased: _canvasScrollState.setReleasedSpeed,
                  inputModel: _isExpanded ? _inputModel : null,
                  scale: _mouseScale,
                  resetCollapseTimer: _resetCollapseTimer,
                )),
          ],
        ),
      );
    }
  }
}
