part of 'server_page.dart';

class _AppIcon extends StatelessWidget {
  const _AppIcon({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: EdgeInsets.symmetric(horizontal: 4.0),
      child: loadIcon(30),
    );
  }
}

class _CloseButton extends StatelessWidget {
  const _CloseButton({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return IconButton(
      onPressed: () {
        windowManager.close();
      },
      icon: const Icon(
        IconFont.close,
        size: 18,
      ),
      splashColor: Colors.transparent,
      hoverColor: Colors.transparent,
    );
  }
}

class _CmHeader extends StatefulWidget {
  final Client client;

  const _CmHeader({Key? key, required this.client}) : super(key: key);

  @override
  State<_CmHeader> createState() => _CmHeaderState();
}

class _CmHeaderState extends State<_CmHeader>
    with AutomaticKeepAliveClientMixin {
  Client get client => widget.client;

  final _time = 0.obs;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(Duration(seconds: 1), (_) {
      if (client.authorized && !client.disconnected) {
        _time.value = _time.value + 1;
      }
    });
    // Call onSelected in post frame callback, since we cannot guarantee that the callback will not call setState.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      gFFI.serverModel.tabController.onSelected?.call(client.id.toString());
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Container(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(10.0),
        gradient: LinearGradient(
          begin: Alignment.topRight,
          end: Alignment.bottomLeft,
          colors: [
            Color(0xff00bfe1),
            Color(0xff0071ff),
          ],
        ),
      ),
      margin: EdgeInsets.symmetric(horizontal: 5.0, vertical: 10.0),
      padding: EdgeInsets.only(
        top: 10.0,
        bottom: 10.0,
        left: 10.0,
        right: 5.0,
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _buildClientAvatar().marginOnly(right: 10.0),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                FittedBox(
                    child: Text(
                  client.name,
                  style: TextStyle(
                    color: Colors.white,
                    fontWeight: FontWeight.bold,
                    fontSize: 20,
                    overflow: TextOverflow.ellipsis,
                  ),
                  maxLines: 1,
                )),
                FittedBox(
                  child: Text(
                    "(${client.peerId})",
                    style: TextStyle(color: Colors.white, fontSize: 14),
                  ),
                ),
                if (client.type_() == ClientType.terminal)
                  FittedBox(
                    child: Text(
                      translate("Terminal"),
                      style: TextStyle(color: Colors.white70, fontSize: 12),
                    ),
                  ),
                if (client.type_() == ClientType.file)
                  FittedBox(
                    child: Text(
                      translate("Transfer file"),
                      style: TextStyle(color: Colors.white70, fontSize: 12),
                    ),
                  ),
                if (client.type_() == ClientType.camera)
                  FittedBox(
                    child: Text(
                      translate("View camera"),
                      style: TextStyle(color: Colors.white70, fontSize: 12),
                    ),
                  ),
                if (client.portForward.isNotEmpty)
                  FittedBox(
                    child: Text(
                      "Port Forward: ${client.portForward}",
                      style: TextStyle(color: Colors.white70, fontSize: 12),
                    ),
                  ),
                SizedBox(height: 10.0),
                FittedBox(
                    child: Row(
                  children: [
                    Text(
                      client.authorized
                          ? client.disconnected
                              ? translate("Disconnected")
                              : translate("Connected")
                          : "${translate("Request access to your device")}...",
                      style: TextStyle(color: Colors.white),
                    ).marginOnly(right: 8.0),
                    if (client.authorized)
                      Obx(
                        () => Text(
                          formatDurationToTime(
                            Duration(seconds: _time.value),
                          ),
                          style: TextStyle(color: Colors.white),
                        ),
                      )
                  ],
                ))
              ],
            ),
          ),
          Offstage(
            offstage: !client.authorized ||
                (client.type_() != ClientType.remote &&
                    client.type_() != ClientType.file &&
                    client.type_() != ClientType.camera),
            child: IconButton(
              onPressed: () => checkClickTime(client.id, () {
                if (client.type_() == ClientType.file) {
                  gFFI.chatModel.toggleCMFilePage();
                } else {
                  gFFI.chatModel
                      .toggleCMChatPage(MessageKey(client.peerId, client.id));
                }
              }),
              icon: SvgPicture.asset(client.type_() == ClientType.file
                  ? 'assets/file_transfer.svg'
                  : 'assets/chat2.svg'),
              splashRadius: kDesktopIconButtonSplashRadius,
            ),
          )
        ],
      ),
    );
  }

  @override
  bool get wantKeepAlive => true;

  Widget _buildClientAvatar() {
    return buildAvatarWidget(
          avatar: client.avatar,
          size: 70,
          borderRadius: 15,
          fallback: _buildInitialAvatar(),
        ) ??
        _buildInitialAvatar();
  }

  Widget _buildInitialAvatar() {
    return Container(
      width: 70,
      height: 70,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: str2color(client.name),
        borderRadius: BorderRadius.circular(15.0),
      ),
      child: Text(
        client.name.isNotEmpty ? client.name[0] : '?',
        style: TextStyle(
          fontWeight: FontWeight.bold,
          color: Colors.white,
          fontSize: 55,
        ),
      ),
    );
  }
}
