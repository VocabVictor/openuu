import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';

import '../../consts.dart';
part 'session_states.dart';
part 'option_states.dart';

// TODO: A lot of dup code.

initSharedStates(String id) {
  PrivacyModeState.init(id);
  BlockInputState.init(id);
  CurrentDisplayState.init(id);
  KeyboardEnabledState.init(id);
  ShowRemoteCursorState.init(id);
  ShowRemoteCursorLockState.init(id);
  RemoteCursorMovedState.init(id);
  FingerprintState.init(id);
  PeerBoolOption.init(id, kOptionZoomCursor, () => false);
  UnreadChatCountState.init(id);
  if (isMobile) ConnectionTypeState.init(id); // desktop in other places
}

removeSharedStates(String id) {
  PrivacyModeState.delete(id);
  BlockInputState.delete(id);
  CurrentDisplayState.delete(id);
  ShowRemoteCursorState.delete(id);
  ShowRemoteCursorLockState.delete(id);
  KeyboardEnabledState.delete(id);
  RemoteCursorMovedState.delete(id);
  FingerprintState.delete(id);
  PeerBoolOption.delete(id, kOptionZoomCursor);
  UnreadChatCountState.delete(id);
  if (isMobile) ConnectionTypeState.delete(id);
}
