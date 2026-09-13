part of 'file_manager_page.dart';

extension _FileManagerViewFileList on _FileManagerViewState {
  Widget _buildFileList(
      BuildContext context, ScrollController scrollController) {
    final fd = controller.directory.value;
    final entries = fd.entries;
    Rx<Entry?> rightClickEntry = Rx(null);

    return ListSearchActionListener(
      node: _keyboardNode,
      buffer: _listSearchBuffer,
      onNext: (buffer) {
        debugPrint("searching next for $buffer");
        assert(buffer.length == 1);
        assert(selectedItems.items.length <= 1);
        var skipCount = 0;
        if (selectedItems.items.isNotEmpty) {
          final index = entries.indexOf(selectedItems.items.first);
          if (index < 0) {
            return;
          }
          skipCount = index + 1;
        }
        var searchResult = entries
            .skip(skipCount)
            .where((element) => element.name.toLowerCase().startsWith(buffer));
        if (searchResult.isEmpty) {
          // cannot find next, lets restart search from head
          debugPrint("restart search from head");
          searchResult = entries.where(
              (element) => element.name.toLowerCase().startsWith(buffer));
        }
        if (searchResult.isEmpty) {
          selectedItems.clear();
          return;
        }
        _jumpToEntry(isLocal, searchResult.first, scrollController,
            kDesktopFileTransferRowHeight);
      },
      onSearch: (buffer) {
        debugPrint("searching for $buffer");
        final selectedEntries = selectedItems;
        final searchResult = entries
            .where((element) => element.name.toLowerCase().startsWith(buffer));
        selectedEntries.clear();
        if (searchResult.isEmpty) {
          selectedItems.clear();
          return;
        }
        _jumpToEntry(isLocal, searchResult.first, scrollController,
            kDesktopFileTransferRowHeight);
      },
      child: Obx(() {
        final entries = controller.directory.value.entries;
        final filteredEntries = _searchText.isNotEmpty
            ? entries.where((element) {
                return element.name.contains(_searchText.value);
              }).toList(growable: false)
            : entries;
        // Keep rows lazy so large directories only build visible list items.
        final rows = filteredEntries.map((entry) {
          final sizeStr =
              entry.isFile ? readableFileSize(entry.size.toDouble()) : "";
          final lastModifiedStr = entry.isDrive
              ? " "
              : "${entry.lastModified().toString().replaceAll(".000", "")}   ";
          var secondaryPosition = RelativeRect.fromLTRB(0, 0, 0, 0);
          onTap() {
            final items = selectedItems;
            // handle double click
            if (_checkDoubleClick(entry)) {
              controller.openDirectory(entry.path);
              items.clear();
              return;
            }
            _onSelectedChanged(items, filteredEntries, entry, isLocal);
          }

          onSecondaryTap() {
            final items = [
              if (!entry.isDrive &&
                  versionCmp(_ffi.ffiModel.pi.version, "1.3.0") >= 0)
                mod_menu.PopupMenuItem(
                  child: Text(translate("Rename")),
                  height: CustomPopupMenuTheme.height,
                  onTap: () {
                    controller.renameAction(entry, isLocal);
                  },
                )
            ];
            if (items.isNotEmpty) {
              rightClickEntry.value = entry;
              final future = mod_menu.showMenu(
                context: context,
                position: secondaryPosition,
                items: items,
              );
              future.then((value) {
                rightClickEntry.value = null;
              });
              future.onError((error, stackTrace) {
                rightClickEntry.value = null;
              });
            }
          }

          onSecondaryTapDown(details) {
            secondaryPosition = RelativeRect.fromLTRB(
                details.globalPosition.dx,
                details.globalPosition.dy,
                details.globalPosition.dx,
                details.globalPosition.dy);
          }

          return Padding(
            padding: EdgeInsets.symmetric(vertical: 1),
            child: Obx(() => Container(
                decoration: BoxDecoration(
                  color: selectedItems.items.contains(entry)
                      ? MyTheme.button
                      : Theme.of(context).cardColor,
                  borderRadius: BorderRadius.all(
                    Radius.circular(5.0),
                  ),
                  border: rightClickEntry.value == entry
                      ? Border.all(
                          color: MyTheme.button,
                          width: 1.0,
                        )
                      : null,
                ),
                key: ValueKey(entry.name),
                height: kDesktopFileTransferRowHeight,
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.spaceAround,
                  children: [
                    Expanded(
                      child: InkWell(
                        child: Row(
                          children: [
                            GestureDetector(
                              child: Obx(
                                () => Container(
                                    width: _nameColWidth.value,
                                    child: Tooltip(
                                      waitDuration: Duration(milliseconds: 500),
                                      message: entry.name,
                                      child: Row(children: [
                                        SizedBox(width: 28, child: Checkbox(
                                          value: selectedItems.items.contains(entry),
                                          onChanged: entry.isDrive ? null : (value) {
                                            if (value == true) { selectedItems.add(entry); } else { selectedItems.remove(entry); }
                                          }, visualDensity: VisualDensity.compact)),
                                        entry.isDrive
                                            ? Image(
                                                    image: iconHardDrive,
                                                    fit: BoxFit.scaleDown,
                                                    color: Theme.of(context)
                                                        .iconTheme
                                                        .color
                                                        ?.withOpacity(0.7))
                                                .paddingAll(4)
                                            : SvgPicture.asset(
                                                entry.isFile
                                                    ? "assets/file.svg"
                                                    : "assets/folder.svg",
                                                colorFilter: svgColor(
                                                    Theme.of(context)
                                                        .tabBarTheme
                                                        .labelColor),
                                              ),
                                        Expanded(
                                            child: Text(entry.name.nonBreaking,
                                                style: TextStyle(
                                                    color: selectedItems.items
                                                            .contains(entry)
                                                        ? Colors.white
                                                        : null),
                                                overflow:
                                                    TextOverflow.ellipsis))
                                      ]),
                                    )),
                              ),
                              onTap: onTap,
                              onSecondaryTap: onSecondaryTap,
                              onSecondaryTapDown: onSecondaryTapDown,
                            ),
                            SizedBox(
                              width: 2.0,
                            ),
                            GestureDetector(
                              child: Obx(
                                () => SizedBox(
                                  width: _modifiedColWidth.value,
                                  child: Tooltip(
                                      waitDuration: Duration(milliseconds: 500),
                                      message: lastModifiedStr,
                                      child: Text(
                                        lastModifiedStr,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                          fontSize: 12,
                                          color: selectedItems.items
                                                  .contains(entry)
                                              ? Colors.white70
                                              : MyTheme.darkGray,
                                        ),
                                      )),
                                ),
                              ),
                              onTap: onTap,
                              onSecondaryTap: onSecondaryTap,
                              onSecondaryTapDown: onSecondaryTapDown,
                            ),
                            // Divider from header.
                            SizedBox(
                              width: 2.0,
                            ),
                            SizedBox(width: 58, child: Text(
                              entry.isDrive ? translate('Drive') : entry.isDirectory ? translate('Folder') : translate('File'),
                              maxLines: 1, overflow: TextOverflow.ellipsis,
                              style: const TextStyle(fontSize: 12))),
                            Expanded(
                              // width: 100,
                              child: GestureDetector(
                                child: Tooltip(
                                  waitDuration: Duration(milliseconds: 500),
                                  message: sizeStr,
                                  child: Text(
                                    sizeStr,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                        fontSize: 10,
                                        color:
                                            selectedItems.items.contains(entry)
                                                ? Colors.white70
                                                : MyTheme.darkGray),
                                  ),
                                ),
                                onTap: onTap,
                                onSecondaryTap: onSecondaryTap,
                                onSecondaryTapDown: onSecondaryTapDown,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ))),
          );
        });

        return Column(
          children: [
            // Header
            Row(
              children: [
                Expanded(child: _buildFileBrowserHeader(context)),
              ],
            ),
            // Body
            Expanded(
              child: ListView.builder(
                controller: scrollController,
                itemExtent: kDesktopFileTransferRowHeight,
                itemBuilder: (context, index) {
                  return rows.elementAt(index);
                },
                itemCount: rows.length,
              ),
            ),
          ],
        );
      }),
    );
  }
}
