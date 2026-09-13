part of 'file_manager_page.dart';

extension _FileManagerPageTransfer on _FileManagerPageState {
  void sendSelection(FileController controller) {
    final destination = controller.getOtherSideDirectoryData();
    final oldIds = jobController.jobTable.map((job) => job.id).toSet();
    controller.sendFiles(controller.selectedItems, destination);
    for (final job in jobController.jobTable.where((job) => !oldIds.contains(job.id))) {
      _transferDestinations[job.id] = destination.directory.path;
    }
    controller.selectedItems.clear();
  }

  Widget transferTable() {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    Widget cell(String value, int flex) => Expanded(flex: flex, child: Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12),
      child: Tooltip(message: value, child: Text(value, maxLines: 1,
        overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 13)))));
    bool active(JobProgress job) => job.state == JobState.inProgress ||
      job.state == JobState.paused || job.state == JobState.none;
    Future<void> cancel(JobProgress job) async {
      await jobController.cancelJob(job.id);
      jobController.jobTable.remove(job);
      _transferDestinations.remove(job.id);
    }
    return Obx(() {
      final jobs = jobController.jobTable.toList();
      final paused = jobs.where((job) => job.state == JobState.paused).toList();
      final pending = jobs.where(active).toList();
      bool canPause(JobProgress job) => _pauseSupported && job.type == JobType.transfer &&
        job.state == JobState.inProgress && (!job.isRemoteToLocal || _ffi.ffiModel.pi.features.fileTransferPause);
      final pausable = jobs.where(canPause).toList();
      return Column(children: [
        SizedBox(height: 48, child: Row(children: [
          Padding(padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Text(t('传输列表', 'Transfers'), style: const TextStyle(fontSize: 17))),
          Expanded(child: SingleChildScrollView(scrollDirection: Axis.horizontal,
            reverse: true, child: Row(children: [
              Tooltip(message: t('暂停下载需要远端也升级到支持暂停的版本', 'Pausing downloads requires an updated remote client'),
                child: TextButton.icon(onPressed: pausable.isEmpty ? null : () async {
                  for (final job in pausable) { await jobController.pauseJob(job.id); }
                }, icon: const Icon(Icons.pause, size: 16), label: Text(t('全部暂停', 'Pause all')))),
              TextButton.icon(onPressed: paused.isEmpty ? null : () {
                for (final job in paused) { jobController.resumeJob(job.id); }
              }, icon: const Icon(Icons.play_arrow_outlined, size: 16), label: Text(t('全部开始', 'Resume all'))),
              TextButton.icon(onPressed: pending.isEmpty ? null : () async {
                for (final job in pending) { await cancel(job); }
              }, icon: const Icon(Icons.close, size: 16), label: Text(t('全部取消', 'Cancel all'))),
              TextButton.icon(onPressed: jobs.any((job) => job.state == JobState.done) ? () {
                for (final job in jobs.where((job) => job.state == JobState.done)) {
                  jobController.jobTable.remove(job);
                  _transferDestinations.remove(job.id);
                }
              } : null, icon: const Icon(Icons.delete_outline, size: 16), label: Text(t('清除完结任务', 'Clear completed'))),
            ]))),
        ])),
        const Divider(height: 1),
        Expanded(child: LayoutBuilder(builder: (context, bounds) => SingleChildScrollView(
          scrollDirection: Axis.horizontal, child: SizedBox(width: max(850.0, bounds.maxWidth),
            child: Column(children: [
              SizedBox(height: 36, child: Row(children: [
                cell(t('名称', 'Name'), 3), cell(t('状态', 'Status'), 3),
                cell(t('大小', 'Size'), 1), cell(t('发送路径', 'Source'), 2),
                cell(t('接收路径', 'Destination'), 2),
                SizedBox(width: 100, child: Text(t('操作', 'Actions'))),
              ])),
              const Divider(height: 1),
              Expanded(child: jobs.isEmpty ? Center(child: Text(translate('No transfers in progress'),
                style: TextStyle(color: Theme.of(context).hintColor))) : ListView.builder(
                itemCount: jobs.length, itemBuilder: (context, index) {
                  final job = jobs[index];
                  return Container(height: 54, decoration: BoxDecoration(border: Border(bottom: BorderSide(color: Theme.of(context).dividerColor.withOpacity(.15)))),
                    child: Row(children: [
                      cell(job.fileName.isEmpty ? job.jobName : job.fileName, 3),
                      Expanded(flex: 3, child: Padding(padding: const EdgeInsets.symmetric(horizontal: 12),
                        child: Column(mainAxisAlignment: MainAxisAlignment.center, crossAxisAlignment: CrossAxisAlignment.start, children: [
                          Text(job.getStatus(), maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 12)),
                          if (job.type == JobType.transfer && job.state == JobState.inProgress)
                            Padding(padding: const EdgeInsets.only(top: 5), child: LinearProgressIndicator(value: job.percent.clamp(0.0, 1.0), minHeight: 3)),
                        ]))),
                      cell(readableFileSize(job.totalSize.toDouble()), 1),
                      cell(job.jobName, 2),
                      cell(_transferDestinations[job.id] ?? (job.to.isEmpty ? '—' : job.to), 2),
                      SizedBox(width: 100, child: Row(children: [
                        if (job.state == JobState.inProgress && job.type == JobType.transfer) IconButton(
                          tooltip: canPause(job) ? t('暂停', 'Pause') : t('需要升级客户端以支持暂停', 'Update the client to support pause'),
                          onPressed: canPause(job) ? () => jobController.pauseJob(job.id) : null,
                          icon: const Icon(Icons.pause, size: 18)),
                        if (job.state == JobState.paused) IconButton(tooltip: translate('Resume'),
                          onPressed: () => jobController.resumeJob(job.id), icon: const Icon(Icons.play_arrow_outlined, size: 18)),
                        IconButton(tooltip: active(job) ? translate('Cancel') : translate('Delete'),
                          onPressed: () async { if (active(job)) { await cancel(job); } else { jobController.jobTable.remove(job); _transferDestinations.remove(job.id); } },
                          icon: Icon(active(job) ? Icons.close : Icons.delete_outline, size: 18)),
                      ])),
                    ]));
                })),
            ]))))),
      ]);
    });
  }

  Widget dropArea(FileManagerView fileView) {
    return DropTarget(
        onDragDone: (detail) =>
            handleDragDone(detail, fileView.controller.isLocal),
        onDragEntered: (enter) {
          _dropMaskVisible.value = true;
        },
        onDragExited: (exit) {
          _dropMaskVisible.value = false;
        },
        child: fileView);
  }

}
