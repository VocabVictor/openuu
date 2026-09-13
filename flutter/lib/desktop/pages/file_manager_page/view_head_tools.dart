part of 'file_manager_page.dart';

/// A head-tool glyph: a 28 hit box around a 16px icon in the secondary text
/// colour, muted when the action is unavailable.
Widget _headToolIcon(String asset, {bool enabled = true}) => SizedBox(
    width: UiSpace.rowActionHitSize,
    height: UiSpace.rowActionHitSize,
    child: Center(
        child: SvgPicture.asset(asset,
            width: UiSpace.rowActionIconSize,
            height: UiSpace.rowActionIconSize,
            colorFilter: svgColor(
                enabled ? UiColor.textSecondary : UiColor.faint))));

extension _FileManagerViewHeadTools on _FileManagerViewState {
  Widget headTools() {
    return Container(
      child: Column(
        children: [
          // symbols
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
                      child: _headToolIcon("assets/arrow.svg"),
                    ),
                    color: Colors.transparent,
                    hoverColor: UiColor.settingsRowHover,
                    onPressed: () {
                      selectedItems.clear();
                      controller.goBack();
                    },
                  ),
                  MenuButton(
                    tooltip: translate('Parent directory'),
                    child: RotatedBox(
                      quarterTurns: 3,
                      child: _headToolIcon("assets/arrow.svg"),
                    ),
                    color: Colors.transparent,
                    hoverColor: UiColor.settingsRowHover,
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
                    height: UiSpace.controlHeight,
                    decoration: BoxDecoration(
                      color: Colors.white,
                      border: Border.all(color: UiColor.inputBorder),
                      borderRadius: BorderRadius.all(
                        Radius.circular(UiSpace.inputRadius),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                          horizontal: UiSpace.s2, vertical: 2),
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
                      child: _headToolIcon("assets/search.svg"),
                      color: Colors.transparent,
                      hoverColor: UiColor.settingsRowHover,
                    );
                  case LocationStatus.pathLocation:
                    return MenuButton(
                      onPressed: null,
                      child: _headToolIcon("assets/close.svg"),
                      color: Colors.transparent,
                      hoverColor: UiColor.settingsRowHover,
                    );
                  case LocationStatus.fileSearchBar:
                    return MenuButton(
                      tooltip: translate('Clear'),
                      onPressed: () {
                        onSearchText("", isLocal);
                        _locationStatus.value = LocationStatus.bread;
                      },
                      child: _headToolIcon("assets/close.svg"),
                      color: Colors.transparent,
                      hoverColor: UiColor.settingsRowHover,
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
                child: _headToolIcon("assets/refresh.svg"),
                color: Colors.transparent,
                hoverColor: UiColor.settingsRowHover,
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
                      child: _headToolIcon("assets/home.svg"),
                      color: Colors.transparent,
                      hoverColor: UiColor.settingsRowHover,
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
                          return UiDialog(
                            title: translate("Create Folder"),
                            onClose: cancel,
                            body: uiDialogField(
                                translate("Please enter the folder name"), name,
                                error: errorText,
                                autoFocus: true,
                                onSubmitted: submit),
                            actions: [
                              UiDialogAction.secondary('Cancel', cancel),
                              UiDialogAction.primary('Ok', submit),
                            ],
                          ).alert(context);
                        });
                      },
                      child: _headToolIcon("assets/folder_new.svg"),
                      color: Colors.transparent,
                      hoverColor: UiColor.settingsRowHover,
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
                          child: _headToolIcon("assets/trash.svg"),
                          color: Colors.transparent,
                          hoverColor: UiColor.settingsRowHover,
                        )),
                    menu(isLocal: isLocal),
                  ],
                ),
              ),
            ],
          ).marginOnly(top: UiSpace.s2)
        ],
      ),
    );
  }
}
