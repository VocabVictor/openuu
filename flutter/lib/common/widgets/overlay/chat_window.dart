part of 'overlay.dart';

/// The mobile action bar floats over the peer's screen, not over a page of ours,
/// so what it has to stand out against is whatever the remote desktop is showing.
/// Resolving it from the palette would make it follow *our* appearance while its
/// actual backdrop is unrelated to it -- in dark mode it would lighten against a
/// backdrop that did not change. It stays a fixed colour for the same reason a
/// road sign does not follow the weather.
///
/// 0x66 is what withOpacity(0.4) resolved to, so the colour is unchanged.
const Color _remoteChromeFill = Color(0x660071FF);

/// The icons on that bar. Not `onPrimary`: `onPrimary` promises legibility on
/// a solid fill of ours, and this bar is 40% transparent over an unknown
/// picture, so no palette member can promise anything about it. White is the
/// honest answer for the same reason the bar's own colour is fixed -- both are
/// chosen against the peer's screen, which our appearance does not describe.
const Color _onRemoteChrome = Colors.white;

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
                      BoxDecoration(border: Border.all(color: UiColor.of(context).border)),
                  child: child);
            });
  }

  Widget _buildMobileAppBar(BuildContext context) {
    return Container(
      color: UiColor.of(context).primaryFill,
      height: 50,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15),
              child: Text(
                translate("Chat"),
                style: TextStyle(
                    color: UiColor.of(context).onPrimary,
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
                  icon: Icon(
                    Icons.keyboard_arrow_down,
                    color: UiColor.of(context).onPrimary,
                  )),
              IconButton(
                  onPressed: () {
                    chatModel.hideChatWindowOverlay();
                    chatModel.hideChatIconOverlay();
                  },
                  icon: Icon(
                    Icons.close,
                    color: UiColor.of(context).onPrimary,
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
                  color: UiColor.of(context).border))),
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
                        size: 20, color: UiColor.of(context).primary),
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
                        color: _remoteChromeFill,
                        borderRadius:
                            BorderRadius.all(Radius.circular(15 * scale))),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceAround,
                      children: [
                        IconButton(
                            color: _onRemoteChrome,
                            onPressed: onBackPressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.arrow_back),
                            iconSize: 24 * scale),
                        IconButton(
                            color: _onRemoteChrome,
                            onPressed: onHomePressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.home),
                            iconSize: 24 * scale),
                        IconButton(
                            color: _onRemoteChrome,
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
                            color: _onRemoteChrome,
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
