part of 'address_book.dart';

extension _AddressBookDropdown on _AddressBookState {
  Widget _buildAbDropdown() {
    if (gFFI.abModel.legacyMode.value) {
      return Offstage();
    }
    final names = gFFI.abModel.addressBookNames();
    if (!names.contains(gFFI.abModel.currentName.value)) {
      return Offstage();
    }
    // order: personal, divider, character order
    // https://pub.dev/packages/dropdown_button2#3-dropdownbutton2-with-items-of-different-heights-like-dividers
    final personalAddressBookName = gFFI.abModel.personalAddressBookName();
    bool contains = names.remove(personalAddressBookName);
    names.sort((a, b) => a.toLowerCase().compareTo(b.toLowerCase()));
    if (contains) {
      names.insert(0, personalAddressBookName);
    }

    Row buildItem(String e, {bool button = false}) {
      return Row(
        children: [
          Expanded(
            child: Tooltip(
                waitDuration: Duration(milliseconds: 500),
                message: gFFI.abModel.translatedName(e),
                child: Text(
                  gFFI.abModel.translatedName(e),
                  style: button ? null : TextStyle(fontSize: 14.0),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  textAlign: button ? TextAlign.center : null,
                )),
          ),
        ],
      );
    }

    final items = names
        .map((e) => DropdownMenuItem(value: e, child: buildItem(e)))
        .toList();
    var menuItemStyleData = MenuItemStyleData(height: 36);
    if (contains && items.length > 1) {
      items.insert(1, DropdownMenuItem(enabled: false, child: Divider()));
      List<double> customHeights = List.filled(items.length, 36);
      customHeights[1] = 4;
      menuItemStyleData = MenuItemStyleData(customHeights: customHeights);
    }
    final TextEditingController textEditingController = TextEditingController();

    final isOptFixed = isOptionFixed(kOptionCurrentAbName);
    return DropdownButton2<String>(
      value: gFFI.abModel.currentName.value,
      onChanged: isOptFixed
          ? null
          : (value) {
              if (value != null) {
                gFFI.abModel.setCurrentName(value);
                bind.setLocalFlutterOption(k: kOptionCurrentAbName, v: value);
              }
            },
      customButton: Obx(() => Container(
            height: stateGlobal.isPortrait.isFalse ? 48 : 40,
            child: Row(children: [
              Expanded(
                  child:
                      buildItem(gFFI.abModel.currentName.value, button: true)),
              Icon(Icons.arrow_drop_down),
            ]),
          )),
      underline: Container(
        height: 0.7,
        color: Theme.of(context).dividerColor.withOpacity(0.1),
      ),
      menuItemStyleData: menuItemStyleData,
      items: items,
      isExpanded: true,
      isDense: true,
      dropdownSearchData: DropdownSearchData(
        searchController: textEditingController,
        searchInnerWidgetHeight: 50,
        searchInnerWidget: Container(
          height: 50,
          padding: const EdgeInsets.only(
            top: 8,
            bottom: 4,
            right: 8,
            left: 8,
          ),
          child: TextFormField(
            expands: true,
            maxLines: null,
            controller: textEditingController,
            decoration: InputDecoration(
              isDense: true,
              contentPadding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 8,
              ),
              hintText: translate('Search'),
              hintStyle: const TextStyle(fontSize: 12),
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
              ),
            ),
          ).workaroundFreezeLinuxMint(),
        ),
        searchMatchFn: (item, searchValue) {
          return item.value
              .toString()
              .toLowerCase()
              .contains(searchValue.toLowerCase());
        },
      ),
    );
  }
}
