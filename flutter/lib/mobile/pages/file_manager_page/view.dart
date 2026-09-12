part of 'file_manager_page.dart';

class FileManagerView extends StatefulWidget {
  final FileController controller;
  final Rx<SelectMode> selectMode;

  FileManagerView({required this.controller, required this.selectMode});

  @override
  State<StatefulWidget> createState() => _FileManagerViewState();
}

class _FileManagerViewState extends State<FileManagerView> {
  final _listScrollController = ScrollController();
  final _breadCrumbScroller = ScrollController();
  late final ascending = Rx<bool>(controller.sortAscending);

  bool get isLocal => widget.controller.isLocal;
  FileController get controller => widget.controller;
  SelectedItems get _selectedItems => widget.controller.selectedItems;

  @override
  void initState() {
    super.initState();
    controller.directory.listen((e) => breadCrumbScrollToEnd());
  }

  @override
  Widget build(BuildContext context) {
    return Column(children: [
      headTools(),
      Expanded(child: Obx(() {
        final entries = controller.directory.value.entries;
        return ListView.builder(
          controller: _listScrollController,
          itemCount: entries.length + 1,
          itemBuilder: (context, index) {
            if (index >= entries.length) {
              return listTail();
            }
            var selected = false;
            if (widget.selectMode.value != SelectMode.none) {
              selected = _selectedItems.items.contains(entries[index]);
            }

            final sizeStr = entries[index].isFile
                ? readableFileSize(entries[index].size.toDouble())
                : "";

            final showCheckBox = () {
              return widget.selectMode.value != SelectMode.none &&
                  widget.selectMode.value.eq(controller.selectedItems.isLocal);
            }();
            return Card(
              child: ListTile(
                leading: entries[index].isDrive
                    ? Padding(
                        padding: EdgeInsets.symmetric(vertical: 8),
                        child: Image(
                            image: iconHardDrive,
                            fit: BoxFit.scaleDown,
                            color: Theme.of(context)
                                .iconTheme
                                .color
                                ?.withOpacity(0.7)))
                    : Icon(
                        entries[index].isFile
                            ? Icons.feed_outlined
                            : Icons.folder,
                        size: 40),
                title: Text(entries[index].name),
                selected: selected,
                subtitle: entries[index].isDrive
                    ? null
                    : Text(
                        "${entries[index].lastModified().toString().replaceAll(".000", "")}   $sizeStr",
                        style: TextStyle(fontSize: 12, color: MyTheme.darkGray),
                      ),
                trailing: entries[index].isDrive
                    ? null
                    : showCheckBox
                        ? Checkbox(
                            value: selected,
                            onChanged: (v) {
                              if (v == null) return;
                              if (v && !selected) {
                                _selectedItems.add(entries[index]);
                              } else if (!v && selected) {
                                _selectedItems.remove(entries[index]);
                              }
                              setState(() {});
                            })
                        : PopupMenuButton<String>(
                            tooltip: "",
                            icon: Icon(Icons.more_vert),
                            itemBuilder: (context) {
                              return [
                                PopupMenuItem(
                                  child: Text(translate("Delete")),
                                  value: "delete",
                                ),
                                PopupMenuItem(
                                  child: Text(translate("Multi Select")),
                                  value: "multi_select",
                                ),
                                PopupMenuItem(
                                  child: Text(translate("Properties")),
                                  value: "properties",
                                  enabled: false,
                                ),
                                if (!entries[index].isDrive &&
                                    versionCmp(gFFI.ffiModel.pi.version,
                                            "1.3.0") >=
                                        0)
                                  PopupMenuItem(
                                    child: Text(translate("Rename")),
                                    value: "rename",
                                  )
                              ];
                            },
                            onSelected: (v) {
                              if (v == "delete") {
                                final items = SelectedItems(isLocal: isLocal);
                                items.add(entries[index]);
                                controller.removeAction(items);
                              } else if (v == "multi_select") {
                                _selectedItems.clear();
                                widget.selectMode.toggle(isLocal);
                                setState(() {});
                              } else if (v == "rename") {
                                controller.renameAction(
                                    entries[index], isLocal);
                              }
                            }),
                onTap: () {
                  if (showCheckBox) {
                    if (selected) {
                      _selectedItems.remove(entries[index]);
                    } else {
                      _selectedItems.add(entries[index]);
                    }
                    setState(() {});
                    return;
                  }
                  if (entries[index].isDirectory || entries[index].isDrive) {
                    controller.openDirectory(entries[index].path);
                  } else {
                    // Perform file-related tasks.
                  }
                },
                onLongPress: entries[index].isDrive
                    ? null
                    : () {
                        _selectedItems.clear();
                        widget.selectMode.toggle(isLocal);
                        if (widget.selectMode.value != SelectMode.none) {
                          _selectedItems.add(entries[index]);
                        }
                        setState(() {});
                      },
              ),
            );
          },
        );
      }))
    ]);
  }

  void breadCrumbScrollToEnd() {
    Future.delayed(Duration(milliseconds: 200), () {
      if (_breadCrumbScroller.hasClients) {
        _breadCrumbScroller.animateTo(
            _breadCrumbScroller.position.maxScrollExtent,
            duration: Duration(milliseconds: 200),
            curve: Curves.fastLinearToSlowEaseIn);
      }
    });
  }

  Widget headTools() => Container(
          child: Row(
        children: [
          Expanded(child: Obx(() {
            final home = controller.options.value.home;
            final isWindows = controller.options.value.isWindows;
            return BreadCrumb(
              items: getPathBreadCrumbItems(controller.shortPath, isWindows,
                  () => controller.goToHomeDirectory(), (list) {
                var path = "";
                if (home.startsWith(list[0])) {
                  // absolute path
                  for (var item in list) {
                    path = PathUtil.join(path, item, isWindows);
                  }
                } else {
                  path += home;
                  for (var item in list) {
                    path = PathUtil.join(path, item, isWindows);
                  }
                }
                controller.openDirectory(path);
              }),
              divider: Icon(Icons.chevron_right),
              overflow: ScrollableOverflow(controller: _breadCrumbScroller),
            );
          })),
          Row(
            children: [
              IconButton(
                icon: Icon(Icons.arrow_back),
                onPressed: controller.goBack,
              ),
              IconButton(
                icon: Icon(Icons.arrow_upward),
                onPressed: controller.goToParentDirectory,
              ),
              PopupMenuButton<SortBy>(
                  tooltip: "",
                  icon: Icon(Icons.sort),
                  itemBuilder: (context) {
                    return SortBy.values
                        .map((e) => PopupMenuItem(
                              child: Text(translate(e.toString())),
                              value: e,
                            ))
                        .toList();
                  },
                  onSelected: (sortBy) {
                    // If selecting the same sort option, flip the order
                    // If selecting a different sort option, use ascending order
                    if (controller.sortBy.value == sortBy) {
                      ascending.value = !controller.sortAscending;
                    } else {
                      ascending.value = true;
                    }
                    controller.changeSortStyle(sortBy,
                        ascending: ascending.value);
                  }),
            ],
          )
        ],
      ));

  Widget listTail() => Obx(() => Container(
        height: 100,
        child: Column(
          children: [
            Padding(
              padding: EdgeInsets.fromLTRB(30, 5, 30, 0),
              child: Text(
                controller.directory.value.path,
                style: TextStyle(color: MyTheme.darkGray),
              ),
            ),
            Padding(
              padding: EdgeInsets.all(2),
              child: Text(
                "${translate("Total")}: ${controller.directory.value.entries.length} ${translate("items")}",
                style: TextStyle(color: MyTheme.darkGray),
              ),
            )
          ],
        ),
      ));

  List<BreadCrumbItem> getPathBreadCrumbItems(String shortPath, bool isWindows,
      void Function() onHome, void Function(List<String>) onPressed) {
    final list = PathUtil.split(shortPath, isWindows);
    final breadCrumbList = [
      BreadCrumbItem(
          content: IconButton(
        icon: Icon(Icons.home_filled),
        onPressed: onHome,
      ))
    ];
    breadCrumbList.addAll(list.asMap().entries.map((e) => BreadCrumbItem(
        content: TextButton(
            child: Text(e.value),
            style:
                ButtonStyle(minimumSize: MaterialStateProperty.all(Size(0, 0))),
            onPressed: () => onPressed(list.sublist(0, e.key + 1))))));
    return breadCrumbList;
  }
}
