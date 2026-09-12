import 'dart:async';
import '../../widgets/quick_launch.dart';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:flutter_hbb/models/state_model.dart';

import '../../../consts.dart';
import '../../../common/widgets/overlay.dart';
import '../../../common/widgets/remote_input.dart';
import '../../../common.dart';
import '../../../common/widgets/dialog.dart';
import '../../../common/widgets/toolbar.dart';
import '../../../models/model.dart';
import '../../../models/input_model.dart';
import '../../../models/platform_model.dart';
import '../../../common/shared_state.dart';
import '../../../utils/image.dart';
import '../../widgets/remote_toolbar.dart';
import '../../widgets/desktop_preview.dart';
import '../../widgets/kb_layout_type_chooser.dart';
import '../../widgets/tabbar_widget.dart';
import '../macos_full_screen_focus_recovery.dart';

import 'package:flutter_hbb/native/custom_cursor.dart'
    if (dart.library.html) 'package:flutter_hbb/web/custom_cursor.dart';
part 'view.dart';
part 'macos.dart';
part 'body.dart';
part 'widgets.dart';
part 'image_paint_scrollbar.dart';
part 'image_paint.dart';
part 'remote_page_widget.dart';

final SimpleWrapper<bool> _firstEnterImage = SimpleWrapper(false);

// Used to skip session close if "move to new window" is clicked.
final Map<String, bool> closeSessionOnDispose = {};
