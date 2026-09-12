part of 'floating_mouse.dart';

extension _MouseBodyScroll on _MouseBodyState {
  Widget _buildScrollUpDown(GlobalKey key, IconData iconData, double s) {
    return Container(
      key: key,
      height: 17 * s,
      child: Icon(
        iconData,
        color: _kDefaultHighlightColor,
        size: 14 * s,
      ),
    );
  }

  Widget _buildScrollMidButton(double s) {
    return Listener(
      onPointerDown: widget.inputModel != null
          ? (event) {
              widget.resetCollapseTimer?.call();
              _setState(() {
                _midDown = true;
                widget.inputModel?.tapDown(MouseButtons.wheel);
              });
            }
          : null,
      onPointerUp: widget.inputModel != null
          ? (event) {
              _setState(() {
                _midDown = false;
                widget.inputModel?.tapUp(MouseButtons.wheel);
                widget.cancelCanvasScroll?.call();
              });
            }
          : null,
      onPointerCancel: widget.inputModel != null
          ? (event) {
              _setState(() {
                _midDown = false;
                widget.inputModel?.tapUp(MouseButtons.wheel);
                widget.cancelCanvasScroll?.call();
              });
            }
          : null,
      onPointerMove: widget.onPointerMoveUpdate,
      behavior: HitTestBehavior.opaque,
      child: Container(
        height: 28 * s,
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 6 * s,
                height: 2 * s,
                color: _kDefaultHighlightColor,
              ),
              SizedBox(height: 3 * s),
              Container(
                width: 8 * s,
                height: 2 * s,
                color: _kDefaultHighlightColor,
              ),
              SizedBox(height: 3 * s),
              Container(
                width: 6 * s,
                height: 2 * s,
                color: _kDefaultHighlightColor,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
