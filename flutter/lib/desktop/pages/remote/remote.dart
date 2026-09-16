import 'dart:async';
import '../../widgets/quick_launch.dart';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:openuu/models/state.dart';

import '../../../consts.dart';
import '../../../common/widgets/overlay.dart';
import '../../../common/widgets/remote_input.dart';
import '../../../common.dart';
import '../../../common/widgets/dialog.dart';
import '../../../common/widgets/toolbar.dart';
import '../../../models/model.dart';
import '../../../models/input.dart';
import '../../../models/platform.dart';
import '../../../common/shared_state.dart';
import '../../../utils/image.dart';
import '../../widgets/remote_toolbar.dart';
import '../../widgets/preview.dart';
import '../../widgets/kb_layout_type_chooser.dart';
import '../../widgets/tabbar.dart';
import '../macos_full_screen_focus_recovery.dart';

import 'package:openuu/native/custom_cursor.dart';
import '../../widgets/session_status_bar.dart';
import '../../widgets/ui_tokens.dart';
part 'view.dart';
part 'status.dart';
part 'macos.dart';
part 'body.dart';
part 'widgets.dart';
part 'image_paint_scrollbar.dart';
part 'image_paint.dart';
part 'remote_page_widget.dart';

final SimpleWrapper<bool> _firstEnterImage = SimpleWrapper(false);

// Used to skip session close if "move to new window" is clicked.
final Map<String, bool> closeSessionOnDispose = {};
