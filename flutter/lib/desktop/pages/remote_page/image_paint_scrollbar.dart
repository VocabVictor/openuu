part of 'remote_page.dart';

extension _ImagePaintScrollbar on _ImagePaintState {
  Widget _buildCrossScrollbarFromLayout(
    BuildContext context,
    Widget child,
    Size layoutSize,
    Size size,
    ScrollController horizontal,
    ScrollController vertical,
  ) {
    var widget = child;
    if (layoutSize.width < size.width) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: horizontal,
          scrollDirection: Axis.horizontal,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Row(
        children: [
          Container(
            width: ((layoutSize.width - size.width) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.height < size.height) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: vertical,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Column(
        children: [
          Container(
            height: ((layoutSize.height - size.height) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.width < size.width) {
      widget = RawScrollbar(
        thickness: kScrollbarThickness,
        thumbColor: Colors.grey,
        controller: horizontal,
        thumbVisibility: false,
        trackVisibility: false,
        notificationPredicate: layoutSize.height < size.height
            ? (notification) => notification.depth == 1
            : defaultScrollNotificationPredicate,
        child: widget,
      );
    }
    if (layoutSize.height < size.height) {
      widget = RawScrollbar(
        thickness: kScrollbarThickness,
        thumbColor: Colors.grey,
        controller: vertical,
        thumbVisibility: false,
        trackVisibility: false,
        child: widget,
      );
    }

    return Container(
      child: widget,
      width: layoutSize.width,
      height: layoutSize.height,
    );
  }
}
