import 'dart:convert';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/widgets/audio_input.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/common/widgets/toolbar.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../../../common.dart';
import '../../../models/model.dart';
import '../../../models/platform_model.dart';
import '../../../common/shared_state.dart';
import '.././popup_menu.dart';
import '.././kb_layout_type_chooser.dart';
import 'package:flutter_hbb/utils/scale.dart';
import 'package:flutter_hbb/common/widgets/custom_scale_base.dart';

part 'chat_voice_menu.dart';
part 'display_menu.dart';
part 'display_menu_options.dart';
part 'draggable_drag.dart';
part 'draggable_show_hide.dart';
part 'edge.dart';
part 'icon_buttons.dart';
part 'keyboard_menu.dart';
part 'menu_buttons.dart';
part 'monitor_menu.dart';
part 'resolutions_menu.dart';
part 'resolutions_menu_actions.dart';
part 'screen_adjustor.dart';
part 'screen_adjustor_linux.dart';
part 'small_menus.dart';
part 'theme.dart';
part 'toolbar_docking.dart';
part 'toolbar_layout.dart';

class RemoteToolbar extends StatefulWidget {
  final String id;
  final FFI ffi;
  final ToolbarState state;
  final Function(int, Function(bool)) onEnterOrLeaveImageSetter;
  final Function(int) onEnterOrLeaveImageCleaner;
  final Function(VoidCallback) setRemoteState;

  RemoteToolbar({
    Key? key,
    required this.id,
    required this.ffi,
    required this.state,
    required this.onEnterOrLeaveImageSetter,
    required this.onEnterOrLeaveImageCleaner,
    required this.setRemoteState,
  }) : super(key: key);

  @override
  State<RemoteToolbar> createState() => _RemoteToolbarState();
}

class _RemoteToolbarState extends State<RemoteToolbar> {
  late Debouncer<int> _debouncerHide;
  bool _isCursorOverImage = false;
  final _fraction = 0.5.obs;
  final _edge = _ToolbarEdge.top.obs;
  final _dragging = false.obs;
  // Live drag preview: where the toolbar would dock if the user dropped now.
  final _previewEdge = Rxn<_ToolbarEdge>();
  final _previewFraction = Rxn<double>();
  // Measured size of the live toolbar, so the preview ghost matches reality
  // (collapsed handle vs expanded toolbar). Updated after every layout pass.
  final _toolbarSize = Rxn<Size>();
  final _toolbarKey = GlobalKey(debugLabel: 'remote_toolbar_root');
  // When false (default), the toolbar stays on the top edge and the drag
  // handle just slides it horizontally — preserving long-standing UX while
  // still fixing the bug where dragging only moved the handle. When true,
  // the user has opted into multi-edge docking with nearest-edge snap.
  // Kept in sync after settings-triggered rebuilds.
  final _multiEdgeEnabled = false.obs;
  final _dockingOptionsInitialized = false.obs;
  bool _pendingDockingOptionSync = false;
  int _dockingOptionSyncSerial = 0;
  int _dragEpoch = 0;

  int get windowId => stateGlobal.windowId;

  void _setFullscreen(bool v) {
    stateGlobal.setFullscreen(v);
    // stateGlobal.fullscreen is RxBool now, no need to call setState.
    // setState(() {});
  }

  RxBool get collapse => widget.state.collapse;
  RxBool get hide => widget.state.hide;
  bool get pin => widget.state.pin;

  PeerInfo get pi => widget.ffi.ffiModel.pi;
  FfiModel get ffiModel => widget.ffi.ffiModel;

  triggerAutoHide() => _debouncerHide.value = _debouncerHide.value + 1;

  void _minimize() async =>
      await WindowController.fromWindowId(windowId).minimize();

  @override
  initState() {
    super.initState();

    final cached = _cachedToolbarDockingOptions(widget.ffi.sessionId);
    final multiEdgeEnabled =
        mainGetLocalBoolOptionSync(kOptionAllowMultiEdgeToolbarDock);
    final shouldResetToTop =
        cached != null && cached.multiEdgeEnabled && !multiEdgeEnabled;
    if (cached != null && !shouldResetToTop) {
      _edge.value = cached.edge;
      _fraction.value = cached.fraction;
      _multiEdgeEnabled.value = multiEdgeEnabled;
      _dockingOptionsInitialized.value = true;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await _syncDockingOptions(force: cached == null || shouldResetToTop);
      // Initialize toolbar states (collapse, hide) from session options
      widget.state.init(widget.ffi.sessionId);
    });

    _debouncerHide = Debouncer<int>(
      Duration(milliseconds: 5000),
      onChanged: _debouncerHideProc,
      initialValue: 0,
    );

    widget.onEnterOrLeaveImageSetter(identityHashCode(this), (enter) {
      if (enter) {
        triggerAutoHide();
        _isCursorOverImage = true;
      } else {
        _isCursorOverImage = false;
      }
    });
  }

  @override
  void didUpdateWidget(covariant RemoteToolbar oldWidget) {
    super.didUpdateWidget(oldWidget);
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await _syncDockingOptions(force: false);
    });
  }

  _debouncerHideProc(int v) {
    if (!pin && collapse.isFalse && _isCursorOverImage && _dragging.isFalse) {
      collapse.value = true;
    }
  }

  @override
  dispose() {
    ++_dockingOptionSyncSerial;
    widget.onEnterOrLeaveImageCleaner(identityHashCode(this));
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      // Wait for initialization to complete to prevent flickering
      if (!widget.state.initialized.value ||
          !_dockingOptionsInitialized.value) {
        return const SizedBox.shrink();
      }
      // If toolbar is hidden, return empty widget
      if (hide.value) {
        return const SizedBox.shrink();
      }
      final edge = _edge.value;
      final isHorizontal = _isHorizontalEdge(edge);

      // Measure the live toolbar after every layout so the preview ghost can
      // match its actual footprint (collapsed handle vs expanded toolbar).
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_dragging.isTrue) return;
        final ro = _toolbarKey.currentContext?.findRenderObject();
        if (ro is RenderBox && ro.hasSize) {
          final s = ro.size;
          if (_toolbarSize.value != s) _toolbarSize.value = s;
        }
      });

      final toolbar = Align(
        alignment: _alignmentForEdge(edge, _fraction.value),
        child: KeyedSubtree(
          key: _toolbarKey,
          child: collapse.isFalse
              ? _buildToolbar(context, edge, isHorizontal)
              : _buildDraggableCollapse(context, edge, isHorizontal),
        ),
      );

      // Always return the Stack — even when not dragging — so the toolbar's
      // position in the Element tree stays stable. Wrapping/unwrapping it
      // mid-drag was killing the Draggable's gesture state.
      return Stack(
        fit: StackFit.expand,
        children: [
          IgnorePointer(
            child: Obx(() {
              final pe = _previewEdge.value;
              final pf = _previewFraction.value;
              if (!_dragging.isTrue || pe == null || pf == null) {
                return const SizedBox.shrink();
              }
              return _buildDragPreview(context, pe, pf, _toolbarSize.value);
            }),
          ),
          toolbar,
        ],
      );
    });
  }

  Widget _buildDragPreview(BuildContext context, _ToolbarEdge edge,
      double fraction, Size? measured) {
    final color = Theme.of(context).colorScheme.primary;
    // Use the measured live toolbar size so collapsed vs expanded looks
    // right. The current orientation may differ from the preview orientation
    // (e.g. dragging a top-docked toolbar toward the left edge), so swap the
    // long/short axes when previewing a different orientation.
    final previewSize = _toolbarSizeForEdge(edge, measured);
    return Align(
      alignment: _alignmentForEdge(edge, fraction),
      child: Container(
        width: previewSize.width,
        height: previewSize.height,
        decoration: BoxDecoration(
          color: color.withOpacity(0.10),
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: color.withOpacity(0.55), width: 1.5),
        ),
      ),
    );
  }

}
