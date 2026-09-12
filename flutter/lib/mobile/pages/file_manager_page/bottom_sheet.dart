part of 'file_manager_page.dart';

class BottomSheetBody extends StatelessWidget {
  BottomSheetBody(
      {required this.leading,
      required this.title,
      required this.text,
      this.onCanceled,
      this.actions});

  final Widget leading;
  final String title;
  final String text;
  final VoidCallback? onCanceled;
  final List<IconButton>? actions;

  @override
  BottomSheet build(BuildContext context) {
    // ignore: no_leading_underscores_for_local_identifiers
    final _actions = actions ?? [];
    return BottomSheet(
      builder: (BuildContext context) {
        return Container(
            height: 65,
            alignment: Alignment.centerLeft,
            decoration: BoxDecoration(
                color: MyTheme.accent50,
                borderRadius: BorderRadius.vertical(top: Radius.circular(10))),
            child: Padding(
              padding: EdgeInsets.symmetric(horizontal: 15),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Row(
                    children: [
                      leading,
                      SizedBox(width: 16),
                      Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(title, style: TextStyle(fontSize: 18)),
                          Text(text,
                              style: TextStyle(fontSize: 14)) // TODO color
                        ],
                      )
                    ],
                  ),
                  Row(children: () {
                    _actions.add(IconButton(
                      icon: Icon(Icons.cancel_outlined),
                      onPressed: onCanceled,
                    ));
                    return _actions;
                  }())
                ],
              ),
            ));
      },
      onClosing: () {},
      // backgroundColor: MyTheme.grayBg,
      enableDrag: false,
    );
  }
}

extension _FileManagerBottomSheet on _FileManagerPageState {
  Widget? bottomSheet() {
    return Obx(() {
      final selectedItems = getActiveSelectedItems();
      final jobTable = model.jobController.jobTable;

      final localLabel = selectedItems?.isLocal == null
          ? ""
          : " [${selectedItems!.isLocal ? translate("Local") : translate("Remote")}]";
      if (!(selectMode.value == SelectMode.none)) {
        final selectedItemsLen =
            "${selectedItems?.items.length ?? 0} ${translate("items")}";
        if (selectedItems == null ||
            selectedItems.items.isEmpty ||
            selectMode.value.eq(showLocal)) {
          return BottomSheetBody(
              leading: Icon(Icons.check),
              title: translate("Selected"),
              text: selectedItemsLen + localLabel,
              onCanceled: () {
                selectedItems?.items.clear();
                selectMode.value = SelectMode.none;
                _setState(() {});
              },
              actions: [
                if (isAndroid &&
                    selectedItems?.isLocal == true &&
                    selectedItems?.items.isNotEmpty == true) ...[
                  if (selectedItems!.items.length == 1 &&
                      selectedItems!.items.single.isFile)
                    IconButton(
                      tooltip: translate("Save as"),
                      icon: Icon(Icons.save_alt),
                      onPressed: () =>
                          _exportFile(selectedItems!.items.single),
                    )
                  else
                    IconButton(
                      tooltip: translate("Export"),
                      icon: Icon(Icons.drive_folder_upload),
                      onPressed: () => _exportItems(selectedItems!),
                    ),
                ],
                IconButton(
                  icon: Icon(Icons.compare_arrows),
                  onPressed: () => _setState(() => showLocal = !showLocal),
                ),
                IconButton(
                  icon: Icon(Icons.delete_forever),
                  onPressed: selectedItems != null
                      ? () async {
                          if (selectedItems.items.isNotEmpty) {
                            await currentFileController
                                .removeAction(selectedItems);
                            selectedItems.items.clear();
                            selectMode.value = SelectMode.none;
                          }
                        }
                      : null,
                )
              ]);
        } else {
          return BottomSheetBody(
              leading: Icon(Icons.input),
              title: translate("Paste here?"),
              text: selectedItemsLen + localLabel,
              onCanceled: () {
                selectedItems.items.clear();
                selectMode.value = SelectMode.none;
                _setState(() {});
              },
              actions: [
                IconButton(
                  icon: Icon(Icons.compare_arrows),
                  onPressed: () => _setState(() => showLocal = !showLocal),
                ),
                IconButton(
                  icon: Icon(Icons.paste),
                  onPressed: () {
                    selectMode.value = SelectMode.none;
                    final otherSide = showLocal
                        ? model.remoteController
                        : model.localController;
                    final thisSideData =
                        DirectoryData(currentDir, currentOptions);
                    otherSide.sendFiles(selectedItems, thisSideData);
                    selectedItems.items.clear();
                    selectMode.value = SelectMode.none;
                  },
                )
              ]);
        }
      }

      if (jobTable.isEmpty) {
        return Offstage();
      }

      // Find the first job that is in progress (the one actually transferring data)
      // Rust backend processes jobs sequentially, so the first inProgress job is the active one
      final activeJob = jobTable
              .firstWhereOrNull((job) => job.state == JobState.inProgress) ??
          jobTable.last;

      switch (activeJob.state) {
        case JobState.inProgress:
          return BottomSheetBody(
            leading: CircularProgressIndicator(),
            title: translate("Waiting"),
            text: "${readableFileSize(activeJob.speed)}/s",
            onCanceled: () {
              model.jobController.cancelJob(activeJob.id);
              jobTable.clear();
            },
          );
        case JobState.done:
          return BottomSheetBody(
            leading: Icon(Icons.check),
            title: "${translate("Successful")}!",
            text: activeJob.display(),
            onCanceled: () => jobTable.clear(),
          );
        case JobState.error:
          return BottomSheetBody(
            leading: Icon(Icons.error),
            title: "${translate("Error")}!",
            text: "",
            onCanceled: () => jobTable.clear(),
          );
        case JobState.none:
          break;
        case JobState.paused:
          // TODO: Handle this case.
          break;
      }
      return Offstage();
    });
  }

  SelectedItems? getActiveSelectedItems() {
    final localSelectedItems = model.localController.selectedItems;
    final remoteSelectedItems = model.remoteController.selectedItems;

    if (localSelectedItems.items.isNotEmpty &&
        remoteSelectedItems.items.isNotEmpty) {
      // assert unreachable
      debugPrint("Wrong SelectedItems state, reset");
      localSelectedItems.clear();
      remoteSelectedItems.clear();
    }

    if (localSelectedItems.items.isEmpty && remoteSelectedItems.items.isEmpty) {
      return null;
    }

    if (localSelectedItems.items.length > remoteSelectedItems.items.length) {
      return localSelectedItems;
    } else {
      return remoteSelectedItems;
    }
  }
}
