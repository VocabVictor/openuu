part of 'consts.dart';

/// Android channel invoke type key
class AndroidChannel {
  static final kStartAction = "start_action";
  static final kGetStartOnBootOpt = "get_start_on_boot_opt";
  static final kSetStartOnBootOpt = "set_start_on_boot_opt";
  static final kSyncAppDirConfigPath = "sync_app_dir";
  static final kPickImportFiles = "pick_import_files";
  static final kImportFile = "import_file";
  static final kExportFile = "export_file";
  static final kPickImportDirectory = "pick_import_directory";
  static final kImportDirectory = "import_directory";
  static final kExportFiles = "export_files";
}

/// The windows targets in the publish time order.
enum WindowsTarget {
  naw, // not a windows target
  xp,
  vista,
  w7,
  w8,
  w8_1,
  w10,
  w11
}
