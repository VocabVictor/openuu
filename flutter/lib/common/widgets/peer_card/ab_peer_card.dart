part of 'peer_card.dart';

class AddressBookPeerCard extends BasePeerCard {
  AddressBookPeerCard({required Peer peer, EdgeInsets? menuPadding, Key? key})
      : super(
            peer: peer,
            tab: PeerTabIndex.ab,
            menuPadding: menuPadding,
            key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context),
      _transferFileAction(context),
      _viewCameraAction(context),
      _terminalAction(context),
    ];

    if (peer.platform == kPeerPlatformWindows) {
      menuItems.add(_terminalRunAsAdminAction(context));
    }

    if (isDesktop && peer.platform != kPeerPlatformAndroid) {
      menuItems.add(_tcpTunnelingAction(context));
    }
    // menuItems.add(await _openNewConnInOptAction(peer.id));
    if (!isWeb) {
      menuItems.add(await _forceAlwaysRelayAction(peer.id));
    }
    if (isWindows && peer.platform == kPeerPlatformWindows) {
      menuItems.add(_rdpAction(context, peer.id));
    }
    if (isWindows) {
      menuItems.add(_createShortCutAction(peer.id));
    }
    if (gFFI.abModel.current.canWrite()) {
      menuItems.add(MenuEntryDivider());
      if (isMobile || isDesktop || isWebDesktop) {
        menuItems.add(_renameAction(peer.id));
      }
      if (gFFI.abModel.current.isPersonal() && peer.hash.isNotEmpty) {
        menuItems.add(_unrememberPasswordAction(peer.id));
      }
      if (!gFFI.abModel.current.isPersonal()) {
        menuItems.add(_changeSharedAbPassword());
      }
      if (gFFI.abModel.currentAbTags.isNotEmpty) {
        menuItems.add(_editTagAction(peer.id));
      }
      menuItems.add(_editNoteAction(peer.id));
    }
    final addressbooks = gFFI.abModel.addressBooksCanWrite();
    if (gFFI.peerTabModel.currentTab == PeerTabIndex.ab.index) {
      addressbooks.remove(gFFI.abModel.currentName.value);
    }
    if (addressbooks.isNotEmpty) {
      menuItems.add(_addToAb(peer));
    }
    menuItems.add(_existIn());
    if (gFFI.abModel.current.canWrite()) {
      menuItems.add(MenuEntryDivider());
      menuItems.add(_removeAction(peer.id));
    }
    return menuItems;
  }

  // address book does not need to update
  @protected
  @override
  void _update() =>
      {}; //gFFI.abModel.pullAb(force: ForcePullAb.current, quiet: true);

  @protected
  MenuEntryBase<String> _editTagAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Edit Tag'),
        style: style,
      ),
      proc: () {
        editAbTagDialog(gFFI.abModel.getPeerTags(id), (selectedTag) async {
          await gFFI.abModel.changeTagForPeers([id], selectedTag);
        });
      },
      padding: super.menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _editNoteAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Edit note'),
        style: style,
      ),
      proc: () {
        editAbPeerNoteDialog(id);
      },
      padding: super.menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  @override
  Future<String> _getAlias(String id) async =>
      gFFI.abModel.find(id)?.alias ?? '';

  MenuEntryBase<String> _changeSharedAbPassword() {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate(
            peer.password.isEmpty ? 'Set shared password' : 'Change Password'),
        style: style,
      ),
      proc: () {
        setSharedAbPasswordDialog(gFFI.abModel.currentName.value, peer);
      },
      padding: super.menuPadding,
      dismissOnClicked: true,
    );
  }

  MenuEntryBase<String> _existIn() {
    final names = gFFI.abModel.idExistIn(peer.id);
    final text = names.join(', ');
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Exist in'),
        style: style,
      ),
      proc: () {
        gFFI.dialogManager.show((setState, close, context) {
          return CustomAlertDialog(
            title: Text(translate('Exist in')),
            content: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [Text(text)]),
            actions: [
              dialogButton(
                "OK",
                icon: Icon(Icons.done_rounded),
                onPressed: close,
              ),
            ],
            onSubmit: close,
            onCancel: close,
          );
        });
      },
      padding: super.menuPadding,
      dismissOnClicked: true,
    );
  }
}

void _rdpDialog(String id) async {
  final maxLength = bind.mainMaxEncryptLen();
  final port = await bind.mainGetPeerOption(id: id, key: 'rdp_port');
  final username = await bind.mainGetPeerOption(id: id, key: 'rdp_username');
  final portController = TextEditingController(text: port);
  final userController = TextEditingController(text: username);
  final passwordController = TextEditingController(
      text: await bind.mainGetPeerOption(id: id, key: 'rdp_password'));
  RxBool secure = true.obs;

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      String port = portController.text.trim();
      String username = userController.text;
      String password = passwordController.text;
      await bind.mainSetPeerOption(id: id, key: 'rdp_port', value: port);
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_username', value: username);
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_password', value: password);
      showToast(translate('Successful'));
      close();
    }

    return CustomAlertDialog(
      title: Text(translate('RDP Settings')),
      content: ConstrainedBox(
        constraints: const BoxConstraints(minWidth: 500),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                isDesktop
                    ? ConstrainedBox(
                        constraints: const BoxConstraints(minWidth: 140),
                        child: Text(
                          "${translate('Port')}:",
                          textAlign: TextAlign.right,
                        ).marginOnly(right: 10))
                    : SizedBox.shrink(),
                Expanded(
                  child: TextField(
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(
                          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$'))
                    ],
                    decoration: InputDecoration(
                        labelText: isDesktop ? null : translate('Port'),
                        hintText: '3389'),
                    controller: portController,
                    autofocus: true,
                  ).workaroundFreezeLinuxMint(),
                ),
              ],
            ).marginOnly(bottom: isDesktop ? 8 : 0),
            Obx(() => Row(
                  children: [
                    stateGlobal.isPortrait.isFalse
                        ? ConstrainedBox(
                            constraints: const BoxConstraints(minWidth: 140),
                            child: Text(
                              "${translate('Username')}:",
                              textAlign: TextAlign.right,
                            ).marginOnly(right: 10))
                        : SizedBox.shrink(),
                    Expanded(
                      child: TextField(
                        decoration: InputDecoration(
                            labelText:
                                isDesktop ? null : translate('Username')),
                        controller: userController,
                      ).workaroundFreezeLinuxMint(),
                    ),
                  ],
                ).marginOnly(bottom: stateGlobal.isPortrait.isFalse ? 8 : 0)),
            Obx(() => Row(
                  children: [
                    stateGlobal.isPortrait.isFalse
                        ? ConstrainedBox(
                            constraints: const BoxConstraints(minWidth: 140),
                            child: Text(
                              "${translate('Password')}:",
                              textAlign: TextAlign.right,
                            ).marginOnly(right: 10))
                        : SizedBox.shrink(),
                    Expanded(
                      child: Obx(() => TextField(
                            obscureText: secure.value,
                            maxLength: maxLength,
                            decoration: InputDecoration(
                                labelText:
                                    isDesktop ? null : translate('Password'),
                                suffixIcon: IconButton(
                                    onPressed: () =>
                                        secure.value = !secure.value,
                                    icon: Icon(secure.value
                                        ? Icons.visibility_off
                                        : Icons.visibility))),
                            controller: passwordController,
                          ).workaroundFreezeLinuxMint()),
                    ),
                  ],
                ))
          ],
        ),
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}
