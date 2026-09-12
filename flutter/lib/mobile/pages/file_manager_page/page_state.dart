part of 'file_manager_page.dart';

class _FileManagerPageState extends State<FileManagerPage> {
  void _setState(VoidCallback fn) => setState(fn);
  final model = gFFI.fileModel;
  final selectMode = SelectMode.none.obs;

  var showLocal = true;

  FileController get currentFileController =>
      showLocal ? model.localController : model.remoteController;
  FileDirectory get currentDir => currentFileController.directory.value;
  DirectoryOptions get currentOptions => currentFileController.options.value;
  final _uniqueKey = UniqueKey();

  @override
  void initState() {
    super.initState();
    gFFI.start(widget.id,
        isFileTransfer: true,
        password: widget.password,
        isSharedPassword: widget.isSharedPassword,
        forceRelay: widget.forceRelay);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    gFFI.ffiModel.updateEventListener(gFFI.sessionId, widget.id);
    WakelockManager.enable(_uniqueKey);
  }

  @override
  void dispose() {
    model.close().whenComplete(() {
      gFFI.close();
      gFFI.dialogManager.dismissAll();
      WakelockManager.disable(_uniqueKey);
    });
    model.jobController.clear();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => WillPopScope(
      onWillPop: () async {
        if (selectMode.value != SelectMode.none) {
          selectMode.value = SelectMode.none;
          setState(() {});
        } else {
          currentFileController.goBack();
        }
        return false;
      },
      child: Scaffold(
        // backgroundColor: MyTheme.grayBg,
        appBar: AppBar(
          leading: Row(children: [
            IconButton(
                icon: Icon(Icons.close),
                onPressed: () => clientClose(gFFI.sessionId, gFFI)),
          ]),
          centerTitle: true,
          title: ToggleSwitch(
            initialLabelIndex: showLocal ? 0 : 1,
            activeBgColor: [MyTheme.idColor],
            inactiveBgColor: Theme.of(context).brightness == Brightness.light
                ? MyTheme.grayBg
                : null,
            inactiveFgColor: Theme.of(context).brightness == Brightness.light
                ? Colors.black54
                : null,
            totalSwitches: 2,
            minWidth: 100,
            fontSize: 15,
            iconSize: 18,
            labels: [translate("Local"), translate("Remote")],
            icons: [Icons.phone_android_sharp, Icons.screen_share],
            onToggle: (index) {
              final current = showLocal ? 0 : 1;
              if (index != current) {
                setState(() => showLocal = !showLocal);
              }
            },
          ),
          actions: [
            PopupMenuButton<String>(
                tooltip: "",
                icon: Icon(Icons.more_vert),
                itemBuilder: (context) {
                  return [
                    PopupMenuItem(
                      child: Row(
                        children: [
                          Icon(Icons.refresh,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Refresh File"))
                        ],
                      ),
                      value: "refresh",
                    ),
                    if (isAndroid)
                      PopupMenuItem(
                        enabled: showLocal && currentDir.path.isNotEmpty,
                        value: "import",
                        child: Row(
                          children: [
                            Icon(Icons.add_to_drive,
                                color: Theme.of(context).iconTheme.color),
                            SizedBox(width: 5),
                            Text(translate("Add"))
                          ],
                        ),
                      ),
                    if (isAndroid)
                      PopupMenuItem(
                        enabled: showLocal && currentDir.path.isNotEmpty,
                        value: "import_folder",
                        child: Row(
                          children: [
                            Icon(Icons.create_new_folder_outlined,
                                color: Theme.of(context).iconTheme.color),
                            SizedBox(width: 5),
                            Text(translate("Import Folder"))
                          ],
                        ),
                      ),
                    if (isAndroid)
                      PopupMenuItem(
                        enabled: showLocal && currentDir.path.isNotEmpty,
                        value: "export_logs",
                        child: Row(
                          children: [
                            Icon(Icons.article_outlined,
                                color: Theme.of(context).iconTheme.color),
                            SizedBox(width: 5),
                            Text(translate("Export Logs"))
                          ],
                        ),
                      ),
                    PopupMenuItem(
                      enabled: currentDir.path != "/",
                      child: Row(
                        children: [
                          Icon(Icons.check,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Multi Select"))
                        ],
                      ),
                      value: "select",
                    ),
                    PopupMenuItem(
                      enabled: currentDir.path != "/",
                      child: Row(
                        children: [
                          Icon(Icons.folder_outlined,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Create Folder"))
                        ],
                      ),
                      value: "folder",
                    ),
                    PopupMenuItem(
                      enabled: currentDir.path != "/",
                      child: Row(
                        children: [
                          Icon(
                              currentOptions.showHidden
                                  ? Icons.check_box_outlined
                                  : Icons.check_box_outline_blank,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Show Hidden Files"))
                        ],
                      ),
                      value: "hidden",
                    )
                  ];
                },
                onSelected: (v) {
                  if (v == "refresh") {
                    currentFileController.refresh();
                  } else if (v == "import") {
                    _importFiles();
                  } else if (v == "import_folder") {
                    _importFolder();
                  } else if (v == "export_logs") {
                    _exportLogs();
                  } else if (v == "select") {
                    model.localController.selectedItems.clear();
                    model.remoteController.selectedItems.clear();
                    selectMode.toggle(showLocal);
                    setState(() {});
                  } else if (v == "folder") {
                    final name = TextEditingController();
                    String? errorText;
                    gFFI.dialogManager.show((setState, close, context) {
                      name.addListener(() {
                        if (errorText != null) {
                          setState(() {
                            errorText = null;
                          });
                        }
                      });
                      return CustomAlertDialog(
                          title: Text(translate("Create Folder")),
                          content: Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              TextFormField(
                                decoration: InputDecoration(
                                  labelText:
                                      translate("Please enter the folder name"),
                                  errorText: errorText,
                                ),
                                controller: name,
                              ).workaroundFreezeLinuxMint(),
                            ],
                          ),
                          actions: [
                            dialogButton("Cancel",
                                onPressed: () => close(false), isOutline: true),
                            dialogButton("OK", onPressed: () {
                              if (name.value.text.isNotEmpty) {
                                if (!PathUtil.validName(
                                    name.value.text,
                                    currentFileController
                                        .options.value.isWindows)) {
                                  setState(() {
                                    errorText =
                                        translate("Invalid folder name");
                                  });
                                  return;
                                }
                                currentFileController.createDir(PathUtil.join(
                                    currentDir.path,
                                    name.value.text,
                                    currentOptions.isWindows));
                                close();
                              }
                            })
                          ]);
                    });
                  } else if (v == "hidden") {
                    currentFileController.toggleShowHidden();
                  }
                }),
          ],
        ),
        body: showLocal
            ? FileManagerView(
                controller: model.localController,
                selectMode: selectMode,
              )
            : FileManagerView(
                controller: model.remoteController,
                selectMode: selectMode,
              ),
        bottomSheet: bottomSheet(),
      ));

}
