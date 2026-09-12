part of 'file_manager_page.dart';

extension _FileManagerImportExport on _FileManagerPageState {
  Future<T> _runAndroidDocumentPicker<T>(Future<T> Function() action) async {
    gFFI.ffiModel.beginAndroidDocumentPicker();
    try {
      return await action();
    } finally {
      gFFI.ffiModel.endAndroidDocumentPicker();
    }
  }

  Future<void> _importFiles() async {
    var imported = 0;
    var failed = false;
    final importController = currentFileController;
    final importDirectory = currentDir.path;
    final importIsWindows = currentOptions.isWindows;
    try {
      final selectedFiles = await _runAndroidDocumentPicker(() =>
          gFFI.invokeMethodWithResult<List<dynamic>>(
              AndroidChannel.kPickImportFiles));
      if (selectedFiles == null || selectedFiles.isEmpty) return;

      for (final selected in selectedFiles) {
        final uri = (selected as Map<dynamic, dynamic>)['uri'] as String?;
        final selectedName = selected['name'] as String?;
        final name = selectedName?.replaceAll('\\', '/').split('/').last;
        if (uri == null ||
            name == null ||
            !PathUtil.validName(name, importIsWindows)) {
          failed = true;
          continue;
        }
        final destination =
          PathUtil.join(importDirectory, name, importIsWindows);
        var overwrite = false;
        if (await File(destination).exists()) {
          final overwriteResult = await model.showFileConfirmDialog(
              translate('Overwrite'), destination, false, false);
          if (overwriteResult == false) break;
          if (overwriteResult != true) continue;
          overwrite = true;
        }
        try {
          final success = await gFFI.invokeMethod(
              AndroidChannel.kImportFile,
              {'uri': uri, 'path': destination, 'overwrite': overwrite});
          if (success == true) {
            imported++;
          } else {
            failed = true;
          }
        } catch (e) {
          failed = true;
          debugPrint('Failed to import $name: $e');
        }
      }
    } catch (e) {
      failed = true;
      debugPrint('Failed to select files for import: $e');
    }
    await importController.refresh();
    if (failed) {
      showToast(translate('Failed'));
    } else if (imported > 0) {
      showToast(translate('Successful'));
    }
  }

  Future<void> _exportFile(Entry entry) async {
    try {
      final exported = await _runAndroidDocumentPicker(() => gFFI
          .invokeMethod(AndroidChannel.kExportFile, {'path': entry.path}));
      if (exported == true) {
        showToast(translate('Successful'));
      }
    } catch (e) {
      debugPrint('Failed to export ${entry.name}: $e');
      showToast(translate('Failed'));
    }
  }

  Future<void> _importFolder() async {
    final importController = currentFileController;
    final importDirectory = currentDir.path;
    final importIsWindows = currentOptions.isWindows;
    try {
      final picked = await _runAndroidDocumentPicker(() =>
          gFFI.invokeMethodWithResult<Map<dynamic, dynamic>>(
              AndroidChannel.kPickImportDirectory));
      if (picked == null || picked.isEmpty) return;
      final uri = picked['uri'] as String?;
      final name =
          (picked['name'] as String?)?.replaceAll('\\', '/').split('/').last;
      if (uri == null ||
          name == null ||
          name == '.' ||
          name == '..' ||
          !PathUtil.validName(name, importIsWindows)) {
        showToast(translate('Failed'));
        return;
      }
      final destination = PathUtil.join(importDirectory, name, importIsWindows);
      final destinationType = await FileSystemEntity.type(destination);
      var overwrite = false;
      if (destinationType == FileSystemEntityType.directory) {
        final overwriteResult = await model.showFileConfirmDialog(
            translate('Overwrite'), destination, false, false);
        if (overwriteResult != true) return;
        overwrite = true;
      } else if (destinationType != FileSystemEntityType.notFound) {
        showToast(translate('Failed'));
        return;
      }
      final success = await gFFI.invokeMethod(AndroidChannel.kImportDirectory,
          {'uri': uri, 'path': destination, 'overwrite': overwrite});
      if (success == true) {
        showToast(translate('Successful'));
      } else {
        showToast(translate('Failed'));
      }
    } catch (e) {
      debugPrint('Failed to import folder: $e');
      showToast(translate('Failed'));
    }
    await importController.refresh();
  }

  Future<void> _exportItems(SelectedItems items) async {
    await _exportPaths(items.items.map((e) => e.path));
  }

  Future<void> _exportLogs() async {
    final home = currentFileController.homePath;
    if (home.isEmpty) {
      showToast(translate('Failed'));
      return;
    }
    final appDir = PathUtil.join(home, appName, false);
    final paths = [
      PathUtil.join(appDir, 'Logs', false),
      PathUtil.join(appDir, 'ScreenRecord', false),
    ].where((p) => File(p).existsSync() || Directory(p).existsSync()).toList();
    if (paths.isEmpty) {
      showToast(translate('Failed'));
      return;
    }
    await _exportPaths(paths);
  }

  Future<void> _exportPaths(Iterable<String> paths) async {
    try {
      final result = await _runAndroidDocumentPicker(() =>
          gFFI.invokeMethodWithResult<Map<dynamic, dynamic>>(
              AndroidChannel.kExportFiles, {'paths': paths.toList()}));
      if (result == null) return;
      final exported = result['exported'] as int? ?? 0;
      final failed = result['failed'] as int? ?? 0;
      if (failed > 0) {
        showToast(translate('Failed'));
      } else if (exported > 0) {
        showToast(translate('Successful'));
      }
    } catch (e) {
      debugPrint('Failed to export paths: $e');
      showToast(translate('Failed'));
    }
  }
}
