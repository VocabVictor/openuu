part of 'port_forward_page.dart';

extension _PortForwardTunnels on _PortForwardPageState {
  buildPrompt(BuildContext context) {
    final pal = UiColor.of(context);
    final type = UiType.of(context);
    return Obx(() => Offstage(
          offstage: pfs.isEmpty && !widget.isRDP,
          child: Container(
              padding: const EdgeInsets.symmetric(
                  horizontal: UiSession.statusBarPaddingX,
                  vertical: UiSpace.s2),
              decoration: BoxDecoration(
                color: pal.surface,
                border: Border(bottom: BorderSide(color: pal.border)),
              ),
              child: Row(children: [
                Container(
                  width: UiSpace.statusDotSize,
                  height: UiSpace.statusDotSize,
                  decoration: BoxDecoration(
                      color: pal.success, shape: BoxShape.circle),
                ),
                const SizedBox(width: UiSpace.statusDotGap),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                      Text(translate('Listening ...'),
                          style: type.rowTitle.copyWith(fontSize: 13)),
                      Text(translate('not_close_tcp_tip'),
                          style: type.caption),
                    ])),
              ])).marginOnly(bottom: UiSpace.s2),
        ));
  }

  buildTunnel(BuildContext context) {
    final type = UiType.of(context);
    text(String label) => Expanded(
        child: Text(translate(label), style: type.caption)
            .marginOnly(left: _kTextLeftMargin));

    return Theme(
      data: Theme.of(context).copyWith(
        colorScheme: Theme.of(context).colorScheme,
      ),
      child: Obx(() => ListView.builder(
          controller: ScrollController(),
          itemCount: pfs.length + 2,
          itemBuilder: ((context, index) {
            if (index == 0) {
              return Container(
                height: UiSpace.groupHeaderHeight,
                color: Theme.of(context).scaffoldBackgroundColor,
                child: Row(children: [
                  text('Local Port'),
                  const SizedBox(width: _kColumn1Width),
                  text('Remote Host'),
                  text('Remote Port'),
                  SizedBox(
                      width: _kColumn4Width,
                      child: Text(translate('Action'), style: type.caption))
                ]),
              );
            } else if (index == 1) {
              return buildTunnelAddRow(context);
            } else {
              return buildTunnelDataRow(context, pfs[index - 2], index - 2);
            }
          }))),
    );
  }

  buildTunnelAddRow(BuildContext context) {
    final pal = UiColor.of(context);
    final type = UiType.of(context);
    var portInputFormatter = [
      FilteringTextInputFormatter.allow(RegExp(
          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$'))
    ];

    return Container(
      height: _kRowHeight,
      decoration:
          BoxDecoration(color: Theme.of(context).colorScheme.background),
      child: Row(children: [
        buildTunnelInputCell(context,
            controller: localPortController,
            inputFormatters: portInputFormatter),
        const SizedBox(
            width: _kColumn1Width, child: Icon(Icons.arrow_forward_sharp)),
        buildTunnelInputCell(context,
            controller: remoteHostController, hint: 'localhost'),
        buildTunnelInputCell(context,
            controller: remotePortController,
            inputFormatters: portInputFormatter),
        SizedBox(
          height: UiSpace.controlHeight,
          child: ElevatedButton(
            style: ElevatedButton.styleFrom(
                backgroundColor: pal.primaryFill,
                foregroundColor: pal.onPrimary,
                elevation: 0,
                padding:
                    const EdgeInsets.symmetric(horizontal: UiSpace.s4),
                shape: RoundedRectangleBorder(
                    borderRadius:
                        BorderRadius.circular(UiSpace.buttonRadius)),
                textStyle: type.button),
            onPressed: () async {
            int? localPort = int.tryParse(localPortController.text);
            int? remotePort = int.tryParse(remotePortController.text);
            if (localPort != null &&
                remotePort != null &&
                (remoteHostController.text.isEmpty ||
                    remoteHostController.text.trim().isNotEmpty)) {
              await bind.sessionAddPortForward(
                  sessionId: _ffi.sessionId,
                  localPort: localPort,
                  remoteHost: remoteHostController.text.trim().isEmpty
                      ? 'localhost'
                      : remoteHostController.text.trim(),
                  remotePort: remotePort);
              localPortController.clear();
              remoteHostController.clear();
              remotePortController.clear();
              refreshTunnelConfig();
            }
          },
            child: Text(translate('Add')),
          ),
        ).marginSymmetric(horizontal: UiSpace.s3),
      ]),
    );
  }

  buildTunnelInputCell(BuildContext context,
      {required TextEditingController controller,
      List<TextInputFormatter>? inputFormatters,
      String? hint}) {
    return Expanded(
      child: Padding(
          padding: const EdgeInsets.all(10.0),
          child: TextField(
              controller: controller,
              inputFormatters: inputFormatters,
              decoration: InputDecoration(
                hintText: hint,
              )).workaroundFreezeLinuxMint()),
    );
  }

  Widget buildTunnelDataRow(BuildContext context, _PortForward pf, int index) {
    final pal = UiColor.of(context);
    final type = UiType.of(context);
    text(String label) => Expanded(
        child: Text(label, style: type.rowTitle)
            .marginOnly(left: _kTextLeftMargin));

    return Container(
      height: _kRowHeight,
      decoration: BoxDecoration(
          color: index % 2 == 0
              ? MyTheme.currentThemeMode() == ThemeMode.dark
                  ? const Color(0xFF202020)
                  : const Color(0xFFF4F5F6)
              : Theme.of(context).colorScheme.background),
      child: Row(children: [
        text(pf.localPort.toString()),
        const SizedBox(width: _kColumn1Width),
        text(pf.remoteHost),
        text(pf.remotePort.toString()),
        SizedBox(
          width: _kColumn4Width,
          child: IconButton(
            iconSize: UiSpace.rowActionIconSize,
            icon: Icon(Icons.close, color: pal.muted),
            onPressed: () async {
              await bind.sessionRemovePortForward(
                  sessionId: _ffi.sessionId, localPort: pf.localPort);
              refreshTunnelConfig();
            },
          ),
        ),
      ]),
    );
  }

  void refreshTunnelConfig() async {
    String peer = bind.mainGetPeerSync(id: widget.id);
    Map<String, dynamic> config = jsonDecode(peer);
    List<dynamic> infos = config['port_forwards'] as List;
    List<_PortForward> result = List.empty(growable: true);
    for (var e in infos) {
      result.add(_PortForward.fromJson(e));
    }
    pfs.value = result;
  }

  buildRdp(BuildContext context) {
    final pal = UiColor.of(context);
    final type = UiType.of(context);
    text1(String label) => Expanded(
        child: Text(translate(label), style: type.caption)
            .marginOnly(left: _kTextLeftMargin));
    text2(String label) => Expanded(
        child: Text(label, style: type.rowTitle)
            .marginOnly(left: _kTextLeftMargin));
    return Theme(
      data: Theme.of(context)
          .copyWith(colorScheme: Theme.of(context).colorScheme),
      child: ListView.builder(
          controller: ScrollController(),
          itemCount: 2,
          itemBuilder: ((context, index) {
            if (index == 0) {
              return Container(
                height: UiSpace.groupHeaderHeight,
                color: Theme.of(context).scaffoldBackgroundColor,
                child: Row(children: [
                  text1('Local Port'),
                  const SizedBox(width: _kColumn1Width),
                  text1('Remote Host'),
                  text1('Remote Port'),
                ]),
              );
            } else {
              return Container(
                height: _kRowHeight,
                decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.background),
                child: Row(children: [
                  Expanded(
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: SizedBox(
                        height: UiSpace.controlHeight,
                        child: ElevatedButton(
                          style: ElevatedButton.styleFrom(
                              backgroundColor: pal.primaryFill,
                              foregroundColor: pal.onPrimary,
                              elevation: 0,
                              padding: const EdgeInsets.symmetric(
                                  horizontal: UiSpace.s4),
                              shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(
                                      UiSpace.buttonRadius)),
                              textStyle: type.button),
                          onPressed: () =>
                              bind.sessionNewRdp(sessionId: _ffi.sessionId),
                          child: Text(translate('New RDP')),
                        ),
                      ).marginOnly(left: _kTextLeftMargin),
                    ),
                  ),
                  const SizedBox(
                      width: _kColumn1Width,
                      child: Icon(Icons.arrow_forward_sharp)),
                  text2('localhost'),
                  text2('RDP'),
                ]),
              );
            }
          })),
    );
  }
}
