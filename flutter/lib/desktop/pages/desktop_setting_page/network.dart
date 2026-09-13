part of 'desktop_setting_page.dart';

class _Network extends StatefulWidget {
  const _Network({Key? key}) : super(key: key);

  @override
  State<_Network> createState() => _NetworkState();
}

class _NetworkState extends State<_Network> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;
  final bool locked = false;

  final scrollController = ScrollController();

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(controller: scrollController, children: [
      preventMouseKeyBuilder(
        block: locked,
        child: Column(children: [
          network(context),
        ]),
      ),
    ]).marginOnly(bottom: _kListViewBottomMargin);
  }

  Widget network(BuildContext context) {
    final hideServer =
        bind.mainGetBuildinOption(key: kOptionHideServerSetting) == 'Y';
    final hideProxy =
        bind.mainGetBuildinOption(key: kOptionHideProxySetting) == 'Y';
    final hideWebSocket = bind.mainGetBuildinOption(key: kOptionHideWebSocketSetting) == 'Y';

    if (hideServer && hideProxy && hideWebSocket) {
      return Offstage();
    }

    // Helper function to create network setting ListTiles
    Widget listTile({
      required IconData icon,
      required String title,
      VoidCallback? onTap,
      Widget? trailing,
      bool showTooltip = false,
      String tooltipMessage = '',
    }) {
      final titleWidget = showTooltip
          ? Row(
              children: [
                Tooltip(
                  waitDuration: Duration(milliseconds: 1000),
                  message: translate(tooltipMessage),
                  child: Row(
                    children: [
                      Text(
                        translate(title),
                        style: TextStyle(fontSize: _kContentFontSize),
                      ),
                      SizedBox(width: 5),
                      Icon(
                        Icons.help_outline,
                        size: 14,
                        color: Theme.of(context)
                            .textTheme
                            .titleLarge
                            ?.color
                            ?.withOpacity(0.7),
                      ),
                    ],
                  ),
                ),
              ],
            )
          : Text(
              translate(title),
              style: TextStyle(fontSize: _kContentFontSize),
            );

      return ListTile(
        leading: Icon(icon, color: _accentColor),
        title: titleWidget,
        enabled: !locked,
        onTap: onTap,
        trailing: trailing,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
        ),
        contentPadding: EdgeInsets.symmetric(horizontal: 16),
        minLeadingWidth: 0,
        horizontalTitleGap: 10,
      );
    }

    Widget switchWidget(IconData icon, String title, String tooltipMessage,
            String optionKey) =>
        listTile(
          icon: icon,
          title: title,
          showTooltip: true,
          tooltipMessage: tooltipMessage,
          trailing: Switch(
            value: mainGetBoolOptionSync(optionKey),
            onChanged: locked || isOptionFixed(optionKey)
                ? null
                : (value) {
                    mainSetBoolOption(optionKey, value);
                    setState(() {});
                  },
          ),
        );

    final outgoingOnly = bind.isOutgoingOnly();

    final divider = const Divider(height: 1, indent: 16, endIndent: 16);
    return _Card(
      title: 'Network',
      children: [
        Container(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (!hideServer)
                listTile(
                  icon: Icons.dns_outlined,
                  title: 'ID/Relay Server',
                  onTap: () => showServerSettings(gFFI.dialogManager, setState),
                ),
              if (!hideProxy && !hideServer) divider,
              if (!hideProxy)
                listTile(
                  icon: Icons.network_ping_outlined,
                  title: 'Socks5/Http(s) Proxy',
                  onTap: changeSocks5Proxy,
                ),
              if (!hideWebSocket && (!hideServer || !hideProxy)) divider,
              if (!hideWebSocket)
                switchWidget(
                    Icons.web_asset_outlined,
                    'Use WebSocket',
                    '${translate('websocket_tip')}\n\n${translate('server-oss-not-support-tip')}',
                    kOptionAllowWebSocket),
              futureBuilder(
                  future: bind.mainIsUsingPublicServer(),
                  hasData: (isUsingPublicServer) {
                    if (isUsingPublicServer) {
                      return Offstage();
                    } else {
                      return Column(
                        children: [
                          if (!hideServer || !hideProxy || !hideWebSocket)
                            divider,
                          switchWidget(
                              Icons.no_encryption_outlined,
                              'Allow insecure TLS fallback',
                              'allow-insecure-tls-fallback-tip',
                              kOptionAllowInsecureTLSFallback),
                          if (!outgoingOnly) divider,
                          if (!outgoingOnly)
                            listTile(
                              icon: Icons.lan_outlined,
                              title: 'Disable UDP',
                              showTooltip: true,
                              tooltipMessage:
                                  '${translate('disable-udp-tip')}\n\n${translate('server-oss-not-support-tip')}',
                              trailing: Switch(
                                value: bind.mainGetOptionSync(
                                        key: kOptionDisableUdp) ==
                                    'Y',
                                onChanged:
                                    locked || isOptionFixed(kOptionDisableUdp)
                                        ? null
                                        : (value) async {
                                            await bind.mainSetOption(
                                                key: kOptionDisableUdp,
                                                value: value ? 'Y' : 'N');
                                            setState(() {});
                                          },
                              ),
                            ),
                        ],
                      );
                    }
                  },
                ),
            ],
          ),
        ),
      ],
    );
  }
}
