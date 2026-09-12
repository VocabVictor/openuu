part of 'file_manager_page.dart';

extension _FileManagerViewHeadTools on _FileManagerViewState {
  Widget headTools() {
    var uploadButtonTapPosition = RelativeRect.fill;
    RxBool isUploadFolder =
        (bind.mainGetLocalOption(key: 'upload-folder-button') == 'Y').obs;
    return Container(
      child: Column(
        children: [
          // symbols
          if (isWeb) PreferredSize(
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      Container(
                          width: 50,
                          height: 50,
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.all(Radius.circular(8)),
                            color: MyTheme.accent,
                          ),
                          padding: EdgeInsets.all(8.0),
                          child: FutureBuilder<String>(
                              future: bind.sessionGetPlatform(
                                  sessionId: _ffi.sessionId,
                                  isRemote: !isLocal),
                              builder: (context, snapshot) {
                                if (snapshot.hasData &&
                                    snapshot.data!.isNotEmpty) {
                                  return getPlatformImage('${snapshot.data}');
                                } else {
                                  return CircularProgressIndicator(
                                    color: Theme.of(context)
                                        .tabBarTheme
                                        .labelColor,
                                  );
                                }
                              })),
                      Text(isLocal
                              ? translate("Local Computer")
                              : translate("Remote Computer"))
                          .marginOnly(left: 8.0)
                    ],
                  ),
                  preferredSize: Size(double.infinity, 70))
              .paddingOnly(bottom: 15),
          // buttons
          Row(
            children: [
              Row(
                children: [
                  MenuButton(
                    tooltip: translate('Back'),
                    padding: EdgeInsets.only(
                      right: 3,
                    ),
                    child: RotatedBox(
                      quarterTurns: 2,
                      child: SvgPicture.asset(
                        "assets/arrow.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                    ),
                    color: Theme.of(context).cardColor,
                    hoverColor: Theme.of(context).hoverColor,
                    onPressed: () {
                      selectedItems.clear();
                      controller.goBack();
                    },
                  ),
                  MenuButton(
                    tooltip: translate('Parent directory'),
                    child: RotatedBox(
                      quarterTurns: 3,
                      child: SvgPicture.asset(
                        "assets/arrow.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                    ),
                    color: Theme.of(context).cardColor,
                    hoverColor: Theme.of(context).hoverColor,
                    onPressed: () {
                      selectedItems.clear();
                      controller.goToParentDirectory();
                    },
                  ),
                ],
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 3.0),
                  child: Container(
                    decoration: BoxDecoration(
                      color: Theme.of(context).cardColor,
                      borderRadius: BorderRadius.all(
                        Radius.circular(8.0),
                      ),
                    ),
                    child: Padding(
                      padding: EdgeInsets.symmetric(vertical: 2.5),
                      child: GestureDetector(
                        onTap: () {
                          _locationStatus.value =
                              _locationStatus.value == LocationStatus.bread
                                  ? LocationStatus.pathLocation
                                  : LocationStatus.bread;
                          Future.delayed(Duration.zero, () {
                            if (_locationStatus.value ==
                                LocationStatus.pathLocation) {
                              _locationNode.requestFocus();
                            }
                          });
                        },
                        child: Obx(
                          () => Container(
                            child: Row(
                              children: [
                                Expanded(
                                    child: _locationStatus.value ==
                                            LocationStatus.bread
                                        ? buildBread()
                                        : buildPathLocation()),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              Obx(() {
                switch (_locationStatus.value) {
                  case LocationStatus.bread:
                    return MenuButton(
                      tooltip: translate('Search'),
                      onPressed: () {
                        _locationStatus.value = LocationStatus.fileSearchBar;
                        Future.delayed(
                            Duration.zero, () => _locationNode.requestFocus());
                      },
                      child: SvgPicture.asset(
                        "assets/search.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                  case LocationStatus.pathLocation:
                    return MenuButton(
                      onPressed: null,
                      child: SvgPicture.asset(
                        "assets/close.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).disabledColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                  case LocationStatus.fileSearchBar:
                    return MenuButton(
                      tooltip: translate('Clear'),
                      onPressed: () {
                        onSearchText("", isLocal);
                        _locationStatus.value = LocationStatus.bread;
                      },
                      child: SvgPicture.asset(
                        "assets/close.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                }
              }),
              MenuButton(
                tooltip: translate('Refresh File'),
                padding: EdgeInsets.only(
                  left: 3,
                ),
                onPressed: () {
                  controller.refresh();
                },
                child: SvgPicture.asset(
                  "assets/refresh.svg",
                  colorFilter:
                      svgColor(Theme.of(context).tabBarTheme.labelColor),
                ),
                color: Theme.of(context).cardColor,
                hoverColor: Theme.of(context).hoverColor,
              ),
            ],
          ),
          Row(
            textDirection: isLocal ? TextDirection.ltr : TextDirection.rtl,
            children: [
              Expanded(
                child: Row(
                  mainAxisAlignment:
                      isLocal ? MainAxisAlignment.start : MainAxisAlignment.end,
                  children: [
                    MenuButton(
                      tooltip: translate('Home'),
                      padding: EdgeInsets.only(
                        right: 3,
                      ),
                      onPressed: () {
                        controller.goToHomeDirectory();
                      },
                      child: SvgPicture.asset(
                        "assets/home.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    ),
                    MenuButton(
                      tooltip: translate('Create Folder'),
                      onPressed: () {
                        final name = TextEditingController();
                        String? errorText;
                        _ffi.dialogManager.show((setState, close, context) {
                          name.addListener(() {
                            if (errorText != null) {
                              _setState(() {
                                errorText = null;
                              });
                            }
                          });
                          submit() {
                            if (name.value.text.isNotEmpty) {
                              if (!PathUtil.validName(name.value.text,
                                  controller.options.value.isWindows)) {
                                _setState(() {
                                  errorText = translate("Invalid folder name");
                                });
                                return;
                              }
                              controller.createDir(PathUtil.join(
                                controller.directory.value.path,
                                name.value.text,
                                controller.options.value.isWindows,
                              ));
                              close();
                            }
                          }

                          cancel() => close(false);
                          return CustomAlertDialog(
                            title: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                SvgPicture.asset("assets/folder_new.svg",
                                    colorFilter: svgColor(MyTheme.accent)),
                                Text(
                                  translate("Create Folder"),
                                ).paddingOnly(
                                  left: 10,
                                ),
                              ],
                            ),
                            content: Column(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                TextFormField(
                                  decoration: InputDecoration(
                                    labelText: translate(
                                      "Please enter the folder name",
                                    ),
                                    errorText: errorText,
                                  ),
                                  controller: name,
                                  autofocus: true,
                                ).workaroundFreezeLinuxMint(),
                              ],
                            ),
                            actions: [
                              dialogButton(
                                "Cancel",
                                icon: Icon(Icons.close_rounded),
                                onPressed: cancel,
                                isOutline: true,
                              ),
                              dialogButton(
                                "Ok",
                                icon: Icon(Icons.done_rounded),
                                onPressed: submit,
                              ),
                            ],
                            onSubmit: submit,
                            onCancel: cancel,
                          );
                        });
                      },
                      child: SvgPicture.asset(
                        "assets/folder_new.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    ),
                    Obx(() => MenuButton(
                          tooltip: translate('Delete'),
                          onPressed: SelectedItems.valid(selectedItems.items)
                              ? () async {
                                  await (controller
                                      .removeAction(selectedItems));
                                  selectedItems.clear();
                                }
                              : null,
                          child: SvgPicture.asset(
                            "assets/trash.svg",
                            colorFilter: svgColor(
                                Theme.of(context).tabBarTheme.labelColor),
                          ),
                          color: Theme.of(context).cardColor,
                          hoverColor: Theme.of(context).hoverColor,
                        )),
                    menu(isLocal: isLocal),
                  ],
                ),
              ),
              if (isWeb)
                Obx(() => ElevatedButton.icon(
                      style: ButtonStyle(
                        padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
                            isLocal
                                ? EdgeInsets.only(left: 10)
                                : EdgeInsets.only(right: 10)),
                        backgroundColor: MaterialStateProperty.all(
                          selectedItems.items.isEmpty
                              ? MyTheme.accent80
                              : MyTheme.accent,
                        ),
                      ),
                      onPressed: () =>
                          {webselectFiles(is_folder: isUploadFolder.value)},
                      label: InkWell(
                        hoverColor: Colors.transparent,
                        splashColor: Colors.transparent,
                        highlightColor: Colors.transparent,
                        focusColor: Colors.transparent,
                        onTapDown: (e) {
                          final x = e.globalPosition.dx;
                          final y = e.globalPosition.dy;
                          uploadButtonTapPosition =
                              RelativeRect.fromLTRB(x, y, x, y);
                        },
                        onTap: () async {
                          final value = await showMenu<bool>(
                              context: context,
                              position: uploadButtonTapPosition,
                              items: [
                                PopupMenuItem<bool>(
                                  value: false,
                                  child: Text(translate('Upload files')),
                                ),
                                PopupMenuItem<bool>(
                                  value: true,
                                  child: Text(translate('Upload folder')),
                                ),
                              ]);
                          if (value != null) {
                            isUploadFolder.value = value;
                            bind.mainSetLocalOption(
                                key: 'upload-folder-button',
                                value: value ? 'Y' : '');
                            webselectFiles(is_folder: value);
                          }
                        },
                        child: Icon(Icons.arrow_drop_down),
                      ),
                      icon: Text(
                        translate(isUploadFolder.isTrue
                            ? 'Upload folder'
                            : 'Upload files'),
                        textAlign: TextAlign.right,
                        style: TextStyle(
                          color: Colors.white,
                        ),
                      ).marginOnly(left: 8),
                    )).marginOnly(left: 16),
              if (isWeb) Obx(() => ElevatedButton.icon(
                    style: ButtonStyle(
                      padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
                          isLocal
                              ? EdgeInsets.only(left: 10)
                              : EdgeInsets.only(right: 10)),
                      backgroundColor: MaterialStateProperty.all(
                        selectedItems.items.isEmpty
                            ? MyTheme.accent80
                            : MyTheme.accent,
                      ),
                    ),
                    onPressed: SelectedItems.valid(selectedItems.items)
                        ? () {
                            final otherSideData =
                                controller.getOtherSideDirectoryData();
                            controller.sendFiles(selectedItems, otherSideData);
                            selectedItems.clear();
                          }
                        : null,
                    icon: isLocal
                        ? Text(
                            translate('Send'),
                            textAlign: TextAlign.right,
                            style: TextStyle(
                              color: selectedItems.items.isEmpty
                                  ? Theme.of(context).brightness ==
                                          Brightness.light
                                      ? MyTheme.grayBg
                                      : MyTheme.darkGray
                                  : Colors.white,
                            ),
                          )
                        : isWeb
                            ? Offstage()
                            : RotatedBox(
                                quarterTurns: 2,
                                child: SvgPicture.asset(
                                  "assets/arrow.svg",
                                  colorFilter: svgColor(
                                      selectedItems.items.isEmpty
                                          ? Theme.of(context).brightness ==
                                                  Brightness.light
                                              ? MyTheme.grayBg
                                              : MyTheme.darkGray
                                          : Colors.white),
                                  alignment: Alignment.bottomRight,
                                ),
                              ),
                    label: isLocal
                        ? SvgPicture.asset(
                            "assets/arrow.svg",
                            colorFilter: svgColor(selectedItems.items.isEmpty
                                ? Theme.of(context).brightness ==
                                        Brightness.light
                                    ? MyTheme.grayBg
                                    : MyTheme.darkGray
                                : Colors.white),
                          )
                        : Text(
                            translate(isWeb ? 'Download' : 'Receive'),
                            style: TextStyle(
                              color: selectedItems.items.isEmpty
                                  ? Theme.of(context).brightness ==
                                          Brightness.light
                                      ? MyTheme.grayBg
                                      : MyTheme.darkGray
                                  : Colors.white,
                            ),
                          ),
                  )),
            ],
          ).marginOnly(top: 8.0)
        ],
      ),
    );
  }
}
