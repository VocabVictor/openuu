part of 'overlay.dart';

class DraggableChatWindow extends StatelessWidget {
  const DraggableChatWindow(
      {Key? key,
      this.position = Offset.zero,
      required this.width,
      required this.height,
      required this.chatModel})
      : super(key: key);

  final Offset position;
  final double width;
  final double height;
  final ChatModel chatModel;

  @override
  Widget build(BuildContext context) {
    if (draggablePositions.chatWindow.isInvalid()) {
      draggablePositions.chatWindow.update(position);
    }
    return isIOS
        ? IOSDraggable(
            position: draggablePositions.chatWindow,
            chatModel: chatModel,
            width: width,
            height: height,
            builder: (context) {
              return Column(
                children: [
                  _buildMobileAppBar(context),
                  Expanded(
                    child: ChatPage(chatModel: chatModel),
                  ),
                ],
              );
            },
          )
        : Draggable(
            checkKeyboard: true,
            checkScreenSize: true,
            position: draggablePositions.chatWindow,
            width: width,
            height: height,
            chatModel: chatModel,
            builder: (context, onPanUpdate) {
              final child = Scaffold(
                resizeToAvoidBottomInset: false,
                appBar: CustomAppBar(
                  onPanUpdate: onPanUpdate,
                  appBar: isDesktop ? _buildDesktopAppBar(context)
                      : _buildMobileAppBar(context),
                ),
                body: ChatPage(chatModel: chatModel),
              );
              return Container(
                  decoration:
                      BoxDecoration(border: Border.all(color: MyTheme.border)),
                  child: child);
            });
  }

  Widget _buildMobileAppBar(BuildContext context) {
    return Container(
      color: Theme.of(context).colorScheme.primary,
      height: 50,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15),
              child: Text(
                translate("Chat"),
                style: const TextStyle(
                    color: Colors.white,
                    fontFamily: 'WorkSans',
                    fontWeight: FontWeight.bold,
                    fontSize: 20),
              )),
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              IconButton(
                  onPressed: () {
                    chatModel.hideChatWindowOverlay();
                  },
                  icon: const Icon(
                    Icons.keyboard_arrow_down,
                    color: Colors.white,
                  )),
              IconButton(
                  onPressed: () {
                    chatModel.hideChatWindowOverlay();
                    chatModel.hideChatIconOverlay();
                  },
                  icon: const Icon(
                    Icons.close,
                    color: Colors.white,
                  ))
            ],
          )
        ],
      ),
    );
  }

  Widget _buildDesktopAppBar(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
          border: Border(
              bottom: BorderSide(
                  color: Theme.of(context).hintColor.withOpacity(0.4)))),
      height: 38,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15, vertical: 8),
              child: Obx(() => Opacity(
                  opacity: chatModel.isWindowFocus.value ? 1.0 : 0.4,
                  child: Row(children: [
                    Icon(Icons.chat_bubble_outline,
                        size: 20, color: Theme.of(context).colorScheme.primary),
                    SizedBox(width: 6),
                    Text(translate("Chat"))
                  ])))),
          Padding(
              padding: EdgeInsets.all(2),
              child: ActionIcon(
                message: 'Close',
                icon: IconFont.close,
                onTap: chatModel.hideChatWindowOverlay,
                isClose: true,
                boxSize: 32,
              ))
        ],
      ),
    );
  }
}

class CustomAppBar extends StatelessWidget implements PreferredSizeWidget {
  final GestureDragUpdateCallback onPanUpdate;
  final Widget appBar;

  const CustomAppBar(
      {Key? key, required this.onPanUpdate, required this.appBar})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return GestureDetector(onPanUpdate: onPanUpdate, child: appBar);
  }

  @override
  Size get preferredSize => const Size.fromHeight(kToolbarHeight);
}

/// floating buttons of back/home/recent actions for android
class DraggableMobileActions extends StatelessWidget {
  DraggableMobileActions(
      {this.onBackPressed,
      this.onRecentPressed,
      this.onHomePressed,
      this.onHidePressed,
      required this.position,
      required this.width,
      required this.height,
      required this.scale});

  final double scale;
  final DraggableKeyPosition position;
  final double width;
  final double height;
  final VoidCallback? onBackPressed;
  final VoidCallback? onHomePressed;
  final VoidCallback? onRecentPressed;
  final VoidCallback? onHidePressed;

  @override
  Widget build(BuildContext context) {
    return Draggable(
        position: position,
        width: scale * width,
        height: scale * height,
        builder: (_, onPanUpdate) {
          return GestureDetector(
              onPanUpdate: onPanUpdate,
              child: Card(
                  color: Colors.transparent,
                  shadowColor: Colors.transparent,
                  child: Container(
                    decoration: BoxDecoration(
                        color: MyTheme.accent.withOpacity(0.4),
                        borderRadius:
                            BorderRadius.all(Radius.circular(15 * scale))),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceAround,
                      children: [
                        IconButton(
                            color: Colors.white,
                            onPressed: onBackPressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.arrow_back),
                            iconSize: 24 * scale),
                        IconButton(
                            color: Colors.white,
                            onPressed: onHomePressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.home),
                            iconSize: 24 * scale),
                        IconButton(
                            color: Colors.white,
                            onPressed: onRecentPressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.more_horiz),
                            iconSize: 24 * scale),
                        const VerticalDivider(
                          width: 0,
                          thickness: 2,
                          indent: 10,
                          endIndent: 10,
                        ),
                        IconButton(
                            color: Colors.white,
                            onPressed: onHidePressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.keyboard_arrow_down),
                            iconSize: 24 * scale),
                      ],
                    ),
                  )));
        });
  }
}
