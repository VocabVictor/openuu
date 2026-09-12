part of 'server_page.dart';

class _PrivilegeBoard extends StatefulWidget {
  final Client client;

  const _PrivilegeBoard({Key? key, required this.client}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _PrivilegeBoardState();
}

class _PrivilegeBoardState extends State<_PrivilegeBoard> {
  late final client = widget.client;
  Widget buildPermissionIcon(bool enabled, IconData iconData,
      Function(bool)? onTap, String tooltipText,
      {required bool canModify}) {
    return Tooltip(
      message: "$tooltipText: ${enabled ? "ON" : "OFF"}",
      waitDuration: Duration.zero,
      child: Container(
        decoration: BoxDecoration(
          color: enabled
              ? (canModify ? MyTheme.accent : MyTheme.accent.withOpacity(0.6))
              : Colors.grey[700],
          borderRadius: BorderRadius.circular(10.0),
        ),
        padding: EdgeInsets.all(8.0),
        child: InkWell(
          onTap: canModify
              ? () =>
                  checkClickTime(widget.client.id, () => onTap?.call(!enabled))
              : null,
          child: Column(
            mainAxisAlignment: MainAxisAlignment.spaceAround,
            children: [
              Expanded(
                child: Icon(
                  iconData,
                  color: Colors.white,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final crossAxisCount = 4;
    final spacing = 10.0;
    final canModifyPermission =
        bind.mainGetBuildinOption(key: kOptionEnablePermChangeInAcceptWindow) !=
            'N';
    return Container(
      width: double.infinity,
      height: 160.0,
      margin: EdgeInsets.all(5.0),
      padding: EdgeInsets.all(5.0),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(10.0),
        color: Theme.of(context).colorScheme.background,
        boxShadow: [
          BoxShadow(
            color: Colors.black.withOpacity(0.2),
            spreadRadius: 1,
            blurRadius: 1,
            offset: Offset(0, 1.5),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Text(
            translate("Permissions"),
            style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
            textAlign: TextAlign.center,
          ).marginOnly(left: 4.0, bottom: 8.0),
          Expanded(
            child: GridView.count(
              crossAxisCount: crossAxisCount,
              padding: EdgeInsets.symmetric(horizontal: spacing),
              mainAxisSpacing: spacing,
              crossAxisSpacing: spacing,
              children: client.type_() == ClientType.camera
                  ? [
                      buildPermissionIcon(
                        client.audio,
                        Icons.volume_up_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "audio",
                              enabled: enabled);
                          setState(() {
                            client.audio = enabled;
                          });
                        },
                        translate('Enable audio'),
                        canModify: canModifyPermission,
                      ),
                      buildPermissionIcon(
                        client.recording,
                        Icons.videocam_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "recording",
                              enabled: enabled);
                          setState(() {
                            client.recording = enabled;
                          });
                        },
                        translate('Enable recording session'),
                        canModify: canModifyPermission,
                      ),
                    ]
                  : [
                      buildPermissionIcon(
                        client.keyboard,
                        Icons.keyboard,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "keyboard",
                              enabled: enabled);
                          setState(() {
                            client.keyboard = enabled;
                          });
                        },
                        translate('Enable keyboard/mouse'),
                        canModify: canModifyPermission,
                      ),
                      buildPermissionIcon(
                        client.clipboard,
                        Icons.assignment_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "clipboard",
                              enabled: enabled);
                          setState(() {
                            client.clipboard = enabled;
                          });
                        },
                        translate('Enable clipboard'),
                        canModify: canModifyPermission,
                      ),
                      buildPermissionIcon(
                        client.audio,
                        Icons.volume_up_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "audio",
                              enabled: enabled);
                          setState(() {
                            client.audio = enabled;
                          });
                        },
                        translate('Enable audio'),
                        canModify: canModifyPermission,
                      ),
                      buildPermissionIcon(
                        client.file,
                        Icons.upload_file_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "file",
                              enabled: enabled);
                          setState(() {
                            client.file = enabled;
                          });
                        },
                        translate('Enable file copy and paste'),
                        canModify: canModifyPermission,
                      ),
                      buildPermissionIcon(
                        client.restart,
                        Icons.restart_alt_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "restart",
                              enabled: enabled);
                          setState(() {
                            client.restart = enabled;
                          });
                        },
                        translate('Enable remote restart'),
                        canModify: canModifyPermission,
                      ),
                      buildPermissionIcon(
                        client.recording,
                        Icons.videocam_rounded,
                        (enabled) {
                          bind.cmSwitchPermission(
                              connId: client.id,
                              name: "recording",
                              enabled: enabled);
                          setState(() {
                            client.recording = enabled;
                          });
                        },
                        translate('Enable recording session'),
                        canModify: canModifyPermission,
                      ),
                      // only windows support block input
                      if (isWindows)
                        buildPermissionIcon(
                          client.blockInput,
                          Icons.block,
                          (enabled) {
                            bind.cmSwitchPermission(
                                connId: client.id,
                                name: "block_input",
                                enabled: enabled);
                            setState(() {
                              client.blockInput = enabled;
                            });
                          },
                          translate('Enable blocking user input'),
                          canModify: canModifyPermission,
                        ),
                      if (bind.mainSupportedPrivacyModeImpls() != '[]')
                        buildPermissionIcon(
                          client.privacyMode,
                          Icons.visibility_off,
                          (enabled) {
                            bind.cmSwitchPermission(
                                connId: client.id,
                                name: "privacy_mode",
                                enabled: enabled);
                            setState(() {
                              client.privacyMode = enabled;
                            });
                          },
                          translate('Enable privacy mode'),
                          canModify: canModifyPermission,
                        )
                    ],
            ),
          ),
        ],
      ),
    );
  }
}
