part of 'server_page.dart';

extension _CmControlPanelAuthorized on _CmControlPanel {
  buildAuthorized(BuildContext context) {
    final bool canElevate = bind.cmCanElevate();
    final model = Provider.of<ServerModel>(context);
    final showElevation = canElevate &&
        model.showElevation &&
        client.type_() == ClientType.remote;
    return Column(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Offstage(
          offstage: !client.inVoiceCall,
          child: Row(
            children: [
              Expanded(
                child: buildButton(context,
                    color: UiColor.of(context).primaryFill,
                    onClick: null, onTapDown: (details) async {
                  final devicesInfo =
                      await AudioInput.getDevicesInfo(true, true);
                  List<String> devices = devicesInfo['devices'] as List<String>;
                  if (devices.isEmpty) {
                    msgBox(
                      gFFI.sessionId,
                      'custom-nocancel-info',
                      'Prompt',
                      'no_audio_input_device_tip',
                      '',
                      gFFI.dialogManager,
                    );
                    return;
                  }

                  String currentDevice = devicesInfo['current'] as String;
                  final x = details.globalPosition.dx;
                  final y = details.globalPosition.dy;
                  final position = RelativeRect.fromLTRB(x, y, x, y);
                  showMenu(
                    context: context,
                    position: position,
                    items: devices
                        .map((d) => PopupMenuItem<String>(
                              value: d,
                              height: 18,
                              padding: EdgeInsets.zero,
                              onTap: () => AudioInput.setDevice(d, true, true),
                              child: IgnorePointer(
                                  child: RadioMenuButton(
                                value: d,
                                groupValue: currentDevice,
                                onChanged: (v) {
                                  if (v != null)
                                    AudioInput.setDevice(v, true, true);
                                },
                                child: Container(
                                  child: Text(
                                    d,
                                    overflow: TextOverflow.ellipsis,
                                    maxLines: 1,
                                  ),
                                  constraints: BoxConstraints(
                                      maxWidth:
                                          kConnectionManagerWindowSizeClosedChat
                                                  .width -
                                              80),
                                ),
                              )),
                            ))
                        .toList(),
                  );
                },
                    icon: Icon(
                      Icons.call_rounded,
                      color: UiColor.of(context).onPrimary,
                      size: UiCm.controlIconSize,
                    ),
                    text: "Audio input",
                    textColor: UiColor.of(context).onPrimary),
              ),
              Expanded(
                child: buildButton(
                  context,
                  color: UiColor.of(context).dangerFill,
                  onClick: () => closeVoiceCall(),
                  icon: Icon(
                    Icons.call_end_rounded,
                    color: UiColor.of(context).onPrimary,
                    size: UiCm.controlIconSize,
                  ),
                  text: "Stop voice call",
                  textColor: UiColor.of(context).onPrimary,
                ),
              )
            ],
          ),
        ),
        Offstage(
          offstage: !client.incomingVoiceCall,
          child: Row(
            children: [
              Expanded(
                child: buildButton(context,
                    color: UiColor.of(context).primaryFill,
                    onClick: () => handleVoiceCall(true),
                    icon: Icon(
                      Icons.call_rounded,
                      color: UiColor.of(context).onPrimary,
                      size: UiCm.controlIconSize,
                    ),
                    text: "Accept",
                    textColor: UiColor.of(context).onPrimary),
              ),
              Expanded(
                child: buildButton(
                  context,
                  color: UiColor.of(context).dangerFill,
                  onClick: () => handleVoiceCall(false),
                  icon: Icon(
                    Icons.phone_disabled_rounded,
                    color: UiColor.of(context).onPrimary,
                    size: UiCm.controlIconSize,
                  ),
                  text: "Dismiss",
                  textColor: UiColor.of(context).onPrimary,
                ),
              )
            ],
          ),
        ),
        Offstage(
          offstage: !client.fromSwitch,
          child: buildButton(context,
              // Not a palette member: this one button is purple to set the
              // "switch sides" action apart from every other action here, and
              // no token means that. It keeps its value until a member does.
              color: Colors.purple,
              onClick: () => handleSwitchBack(context),
              icon: Icon(Icons.reply, color: UiColor.of(context).onPrimary),
              text: "Switch Sides",
              textColor: UiColor.of(context).onPrimary),
        ),
        Offstage(
          offstage: !showElevation,
          child: buildButton(
            context,
            color: UiColor.of(context).warning,
            onClick: () {
              handleElevate(context);
              windowManager.minimize();
            },
            icon: Icon(
              Icons.security_rounded,
              color: UiColor.of(context).onWarning,
              size: UiCm.controlIconSize,
            ),
            text: 'Elevate',
            textColor: UiColor.of(context).onWarning,
          ),
        ),
        Row(
          children: [
            Expanded(
              child: buildButton(context,
                  color: UiColor.of(context).dangerFill,
                  onClick: handleDisconnect,
                  text: 'Disconnect',
                  icon: Icon(
                    Icons.link_off_rounded,
                    color: UiColor.of(context).onPrimary,
                    size: UiCm.controlIconSize,
                  ),
                  textColor: UiColor.of(context).onPrimary),
            ),
          ],
        )
      ],
    ).marginOnly(bottom: buttonBottomMargin);
  }
}
