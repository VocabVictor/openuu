part of 'file_manager_page.dart';

extension _FileManagerViewHeader on _FileManagerViewState {
  Widget _buildFileBrowserHeader(BuildContext context) {
    final padding = EdgeInsets.all(1.0);
    return SizedBox(
      key: _globalHeaderKey,
      height: kDesktopFileTransferHeaderHeight,
      child: Row(
        children: [
          Obx(
            () => headerItemFunc(
                _nameColWidth.value, SortBy.name, translate("Name")),
          ),
          DraggableDivider(
            axis: Axis.vertical,
            onPointerMove: (dx) =>
                _onDrag(dx, _nameColWidth, _modifiedColWidth),
            padding: padding,
          ),
          Obx(
            () => headerItemFunc(_modifiedColWidth.value, SortBy.modified,
                translate("Modified")),
          ),
          DraggableDivider(
              axis: Axis.vertical,
              onPointerMove: (dx) =>
                  _onDrag(dx, _modifiedColWidth, _sizeColWidth),
              padding: padding),
          SizedBox(width: 58, child: Text(translate('Type'), style: const TextStyle(fontSize: 13))),
          Expanded(
              child: headerItemFunc(
                  _sizeColWidth.value, SortBy.size, translate("Size")))
        ],
      ),
    );
  }

  Widget headerItemFunc(double? width, SortBy sortBy, String name) {
    final headerTextStyle =
        Theme.of(context).dataTableTheme.headingTextStyle ?? TextStyle();
    return ObxValue<Rx<bool?>>(
        (ascending) => InkWell(
              onTap: () {
                if (ascending.value == null) {
                  ascending.value = true;
                } else {
                  ascending.value = !ascending.value!;
                }
                controller.changeSortStyle(sortBy,
                    isLocal: isLocal, ascending: ascending.value!);
              },
              child: SizedBox(
                width: width,
                height: kDesktopFileTransferHeaderHeight,
                child: Row(
                  children: [
                    if (sortBy == SortBy.name) SizedBox(width: 28, child: Obx(() {
                      final entries = controller.directory.value.entries.where((entry) => !entry.isDrive).toList();
                      return Checkbox(value: entries.isNotEmpty && entries.every((entry) => selectedItems.items.contains(entry)),
                        onChanged: entries.isEmpty ? null : (value) {
                          if (value == true) { for (final entry in entries) { selectedItems.add(entry); } } else { selectedItems.clear(); }
                        }, visualDensity: VisualDensity.compact);
                    })),
                    Expanded(
                      child: Text(
                        name,
                        style: headerTextStyle,
                        overflow: TextOverflow.ellipsis,
                      ).marginOnly(left: 4),
                    ),
                    ascending.value != null
                        ? Icon(
                            ascending.value!
                                ? Icons.keyboard_arrow_up_rounded
                                : Icons.keyboard_arrow_down_rounded,
                          )
                        : SizedBox()
                  ],
                ),
              ),
            ), () {
      if (controller.sortBy.value == sortBy) {
        return controller.sortAscending.obs;
      } else {
        return Rx<bool?>(null);
      }
    }());
  }
}
