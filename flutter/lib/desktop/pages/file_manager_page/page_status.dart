part of 'file_manager_page.dart';

extension _FileManagerPageStatus on _FileManagerPageState {
  /// transfer status list
  /// watch transfer status
  Widget statusList() {
    Widget getIcon(JobProgress job) {
      final color = Theme.of(context).tabBarTheme.labelColor;
      switch (job.type) {
        case JobType.deleteDir:
        case JobType.deleteFile:
          return Icon(Icons.delete_outline, color: color);
        default:
          return Transform.rotate(
            angle: job.isRemoteToLocal ? pi : 0,
            child: Icon(Icons.arrow_forward_ios, color: color),
          );
      }
    }

    statusListView(List<JobProgress> jobs) => ListView.builder(
          controller: ScrollController(),
          itemBuilder: (BuildContext context, int index) {
            final item = jobs[index];
            final status = item.getStatus();
            return Padding(
              padding: const EdgeInsets.only(bottom: 5),
              child: generateCard(
                Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        getIcon(item)
                            .marginSymmetric(horizontal: 10, vertical: 12),
                        Expanded(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Tooltip(
                                waitDuration: Duration(milliseconds: 500),
                                message: item.jobName,
                                child: ExtendedText(
                                  item.jobName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  overflowWidget: TextOverflowWidget(
                                      child: Text("..."),
                                      position: TextOverflowPosition.start),
                                ),
                              ),
                              Tooltip(
                                waitDuration: Duration(milliseconds: 500),
                                message: status,
                                child: Text(status,
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: MyTheme.darkGray,
                                    )).marginOnly(top: 6),
                              ),
                              Offstage(
                                offstage: item.type != JobType.transfer ||
                                    item.state != JobState.inProgress,
                                child: LinearPercentIndicator(
                                  animateFromLastPercent: true,
                                  center: SizedBox.expand(
                                    child: ShaderMask(
                                      blendMode: BlendMode.srcATop,
                                      shaderCallback: (bounds) =>
                                          LinearGradient(
                                        colors: [
                                          Colors.white,
                                          Colors.transparent,
                                        ],
                                        stops: [item.percent, item.percent],
                                      ).createShader(bounds),
                                      child: FittedBox(
                                        fit: BoxFit.scaleDown,
                                        child: Text.rich(
                                          TextSpan(
                                            text: item.percentText,
                                            children: [
                                              if (item.recvJobRes)
                                                TextSpan(
                                                  text:
                                                      ' ${readableFileSize(item.speed)}/s',
                                                  style: TextStyle(
                                                    fontSize: 12,
                                                    fontWeight: FontWeight.w300,
                                                    color: MyTheme.darkGray,
                                                  ),
                                                ),
                                            ],
                                          ),
                                        ),
                                      ),
                                    ),
                                  ),
                                  barRadius: Radius.circular(15),
                                  percent: item.percent,
                                  progressColor: MyTheme.accent,
                                  backgroundColor: Theme.of(context).hoverColor,
                                  lineHeight: kDesktopFileTransferRowHeight,
                                ).paddingSymmetric(vertical: 8),
                              ),
                            ],
                          ),
                        ),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: [
                            Offstage(
                              offstage: item.state != JobState.paused,
                              child: MenuButton(
                                tooltip: translate("Resume"),
                                onPressed: () {
                                  jobController.resumeJob(item.id);
                                },
                                child: SvgPicture.asset(
                                  "assets/refresh.svg",
                                  colorFilter: svgColor(Colors.white),
                                ),
                                color: MyTheme.accent,
                                hoverColor: MyTheme.accent80,
                              ),
                            ),
                            MenuButton(
                              tooltip: translate("Delete"),
                              child: SvgPicture.asset(
                                "assets/close.svg",
                                colorFilter: svgColor(Colors.white),
                              ),
                              onPressed: () {
                                jobController.jobTable.removeAt(index);
                                jobController.cancelJob(item.id);
                              },
                              color: MyTheme.accent,
                              hoverColor: MyTheme.accent80,
                            ),
                          ],
                        ).marginAll(12),
                      ],
                    ),
                  ],
                ),
              ),
            );
          },
          itemCount: jobController.jobTable.length,
        );

    return PreferredSize(
      preferredSize: const Size(200, double.infinity),
      child: Container(
          margin: const EdgeInsets.only(top: 16.0, bottom: 16.0, right: 16.0),
          padding: const EdgeInsets.all(8.0),
          child: Obx(
            () => jobController.jobTable.isEmpty
                ? generateCard(
                    Center(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          SvgPicture.asset(
                            "assets/transfer.svg",
                            colorFilter: svgColor(
                                Theme.of(context).tabBarTheme.labelColor),
                            height: 40,
                          ).paddingOnly(bottom: 10),
                          Text(
                            translate("No transfers in progress"),
                            textAlign: TextAlign.center,
                            textScaler: TextScaler.linear(1.20),
                            style: TextStyle(
                                color:
                                    Theme.of(context).tabBarTheme.labelColor),
                          ),
                        ],
                      ),
                    ),
                  )
                : statusListView(jobController.jobTable),
          )),
    );
  }

  void handleDragDone(DropDoneDetails details, bool isLocal) {
    if (isLocal) {
      // ignore local
      return;
    }
    final items = SelectedItems(isLocal: false);
    for (var file in details.files) {
      final f = File(file.path);
      items.add(Entry()
        ..path = file.path
        ..name = file.name
        ..size = FileSystemEntity.isDirectorySync(f.path) ? 0 : f.lengthSync());
    }
    final otherSideData = model.localController.directoryData();
    model.remoteController.sendFiles(items, otherSideData);
  }
}
