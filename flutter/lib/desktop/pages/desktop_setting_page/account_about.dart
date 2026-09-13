part of 'desktop_setting_page.dart';

class _Account extends StatefulWidget {
  const _Account({Key? key}) : super(key: key);

  @override
  State<_Account> createState() => _AccountState();
}

class _AccountState extends State<_Account> {
  @override
  Widget build(BuildContext context) {
    final scrollController = ScrollController();
    return ListView(
      controller: scrollController,
      children: [
        if (isWindows && !bind.isIncomingOnly())
          Card(
              margin: const EdgeInsets.only(left: _kCardLeftMargin, top: 8),
              child: Column(children: [
                _settingRow(context, 'Account',
                    SizedBox(width: 220, child: accountAction())),
                useInfo(),
              ]))
        else
          _Card(title: 'Account', children: [accountAction(), useInfo()]),
      ],
    ).marginOnly(bottom: _kListViewBottomMargin);
  }

  Widget accountAction() {
    return Obx(() => _Button(
        gFFI.userModel.userName.value.isEmpty
            ? 'Login'
            : '${translate('Logout')} (${gFFI.userModel.accountLabelWithHandle})',
        () => {
              gFFI.userModel.userName.value.isEmpty
                  ? loginDialog()
                  : logOutConfirmDialog()
            }));
  }

  Widget useInfo() {
    return Obx(() => Offstage(
          offstage: gFFI.userModel.userName.value.isEmpty,
          child: Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Builder(builder: (context) {
              final avatarWidget = _buildUserAvatar();
              return Row(
                children: [
                  if (avatarWidget != null) avatarWidget,
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          gFFI.userModel.displayNameOrUserName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                            fontSize: 16,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const SizedBox(height: 2),
                        SelectionArea(
                          child: Text(
                            '@${gFFI.userModel.userName.value}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: 13,
                              color:
                                  Theme.of(context).textTheme.bodySmall?.color,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              );
            }),
          ),
        )).marginOnly(left: 18, top: 16);
  }

  Widget? _buildUserAvatar() {
    // Resolve relative avatar path at display time
    final avatar =
        bind.mainResolveAvatarUrl(avatar: gFFI.userModel.avatar.value);
    return buildAvatarWidget(
      avatar: avatar,
      size: 44,
    );
  }
}

class _Checkbox extends StatefulWidget {
  final String label;
  final bool Function() getValue;
  final Future<void> Function(bool) setValue;

  const _Checkbox(
      {Key? key,
      required this.label,
      required this.getValue,
      required this.setValue})
      : super(key: key);

  @override
  State<_Checkbox> createState() => _CheckboxState();
}

class _CheckboxState extends State<_Checkbox> {
  var value = false;

  @override
  initState() {
    super.initState();
    value = widget.getValue();
  }

  @override
  Widget build(BuildContext context) {
    onChanged(bool b) async {
      await widget.setValue(b);
      setState(() {
        value = widget.getValue();
      });
    }

    return GestureDetector(
      child: Row(
        children: [
          Checkbox(
            value: value,
            onChanged: (_) => onChanged(!value),
          ).marginOnly(right: 5),
          Expanded(
            child: Text(translate(widget.label)),
          )
        ],
      ).marginOnly(left: _kCheckBoxLeftMargin),
      onTap: () => onChanged(!value),
    );
  }
}

class _About extends StatefulWidget {
  const _About({Key? key}) : super(key: key);

  @override
  State<_About> createState() => _AboutState();
}

class _AboutState extends State<_About> {
  @override
  Widget build(BuildContext context) {
    return futureBuilder(future: () async {
      final license = await bind.mainGetLicense();
      final version = await bind.mainGetVersion();
      final buildDate = await bind.mainGetBuildDate();
      final fingerprint = await bind.mainGetFingerprint();
      final myId = await bind.mainGetMyId();
      return {
        'license': license,
        'version': version,
        'buildDate': buildDate,
        'fingerprint': fingerprint,
        'myId': myId
      };
    }(), hasData: (data) {
      final license = data['license'].toString();
      final version = data['version'].toString();
      final buildDate = data['buildDate'].toString();
      final fingerprint = data['fingerprint'].toString();
      final myId = data['myId'].toString();
      const linkStyle = TextStyle(decoration: TextDecoration.underline);
      final scrollController = ScrollController();
      if (isWindows && !bind.isIncomingOnly()) {
        Widget info(String label, String value) =>
            _settingRow(context, label, const SizedBox.shrink(),
                description: value);
        return ListView(controller: scrollController, children: [
          Card(
              margin: const EdgeInsets.only(left: _kCardLeftMargin, top: 8),
              child: Column(children: [
                _settingRow(
                    context,
                    'OpenUU',
                    const BrandIcon(),
                    description: 'Remote desktop'),
                info('Version', version),
                info('Build Date', buildDate),
                info('Fingerprint', fingerprint),
                info('ID', myId),
                _settingRow(
                    context,
                    'Website',
                    IconButton(
                        icon: const Icon(Icons.open_in_new, size: 20),
                        onPressed: () =>
                            launchUrlString('https://github.com/VocabVictor/openuu'))),
                Padding(
                    padding: const EdgeInsets.all(20),
                    child: SelectableText(
                        'Copyright © ${DateTime.now().year} Purslane Tech Pte. Ltd.\n$license',
                        style: const TextStyle(
                            fontSize: 12,
                            height: 1.5,
                            color: Color(0xff858b95)))),
              ])),
        ]);
      }
      return SingleChildScrollView(
        controller: scrollController,
        child: _Card(title: translate('About RustDesk'), children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const SizedBox(
                height: 8.0,
              ),
              SelectionArea(
                  child: Text('${translate('Version')}: $version')
                      .marginSymmetric(vertical: 4.0)),
              SelectionArea(
                  child: Text('${translate('Build Date')}: $buildDate')
                      .marginSymmetric(vertical: 4.0)),
              SelectionArea(
                    child: Text('${translate('Fingerprint')}: $fingerprint')
                        .marginSymmetric(vertical: 4.0)),
              SelectionArea(
                  child: Text('${translate('ID')}: $myId')
                      .marginSymmetric(vertical: 4.0)),
              InkWell(
                  onTap: () {
                    launchUrlString('https://github.com/VocabVictor/openuu');
                  },
                  child: Text(
                    translate('Website'),
                    style: linkStyle,
                  ).marginSymmetric(vertical: 4.0)),
              Container(
                decoration: const BoxDecoration(color: Color(0xFF2c8cff)),
                padding:
                    const EdgeInsets.symmetric(vertical: 24, horizontal: 8),
                child: SelectionArea(
                    child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Copyright © ${DateTime.now().toString().substring(0, 4)} Purslane Tech Pte. Ltd.\n$license',
                            style: const TextStyle(color: Colors.white),
                          ),
                          Text(
                            translate('Slogan_tip'),
                            style: TextStyle(
                                fontWeight: FontWeight.w800,
                                color: Colors.white),
                          )
                        ],
                      ),
                    ),
                  ],
                )),
              ).marginSymmetric(vertical: 4.0)
            ],
          ).marginOnly(left: _kContentHMargin)
        ]),
      );
    });
  }
}
