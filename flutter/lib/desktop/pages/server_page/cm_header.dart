part of 'server_page.dart';

class _AppIcon extends StatelessWidget {
  const _AppIcon({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: UiCm.titleBarPaddingX),
      child: loadIcon(UiCm.appIconSize),
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
      icon: Icon(
        IconFont.close,
        size: UiCm.titleBarIconSize,
        color: UiColor.of(context).textSecondary,
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

  /// The kind of session being asked for, shown under the id. Null for a
  /// plain remote-control request, which the status line already describes.
  String? _sessionKind() {
    switch (client.type_()) {
      case ClientType.terminal:
        return translate("Terminal");
      case ClientType.file:
        return translate("Transfer file");
      case ClientType.camera:
        return translate("View camera");
      default:
        return client.portForward.isNotEmpty
            ? "Port Forward: ${client.portForward}"
            : null;
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final kind = _sessionKind();
    return Container(
      decoration: BoxDecoration(
        color: UiColor.of(context).panelBg,
        borderRadius: BorderRadius.circular(UiCm.controlRadius),
        border: Border.all(color: UiColor.of(context).border),
      ),
      margin: const EdgeInsets.symmetric(
          horizontal: UiSpace.s1, vertical: UiSpace.s2),
      padding: const EdgeInsets.symmetric(
          horizontal: UiCm.bannerPaddingX, vertical: UiCm.bannerPaddingY),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _buildClientAvatar().marginOnly(right: UiCm.bannerGap),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  client.name,
                  style: UiType.of(context).sectionTitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                Text(
                  "(${client.peerId})",
                  style: UiType.of(context).caption,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                if (kind != null)
                  Text(
                    kind,
                    style: UiType.of(context).caption,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                const SizedBox(height: UiSpace.s3),
                Row(
                  children: [
                    Flexible(
                      child: Text(
                        client.authorized
                            ? client.disconnected
                                ? translate("Disconnected")
                                : translate("Connected")
                            : "${translate("Request access to your device")}...",
                        style: UiType.of(context).rowTitle,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ).marginOnly(right: UiSpace.s2),
                    ),
                    if (client.authorized)
                      Obx(
                        () => Text(
                          formatDurationToTime(
                            Duration(seconds: _time.value),
                          ),
                          style: UiType.of(context).caption,
                        ),
                      )
                  ],
                )
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
          size: UiCm.avatarSize,
          borderRadius: UiCm.avatarRadius,
          fallback: _buildInitialAvatar(),
        ) ??
        _buildInitialAvatar();
  }

  Widget _buildInitialAvatar() {
    return Container(
      width: UiCm.avatarSize,
      height: UiCm.avatarSize,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: str2color(client.name),
        borderRadius: BorderRadius.circular(UiCm.avatarRadius),
      ),
      child: Text(
        client.name.isNotEmpty ? client.name[0] : '?',
        style: const TextStyle(
          fontWeight: FontWeight.w600,
          // The avatar's fill is derived from the peer's name (str2color),
          // not from the palette, so its ink is not a palette member either:
          // it has to be legible on whatever hue that function returns.
          color: Colors.white,
          fontSize: UiCm.avatarInitialSize,
        ),
      ),
    );
  }
}
