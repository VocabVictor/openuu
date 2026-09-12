part of 'shared_state.dart';

class ShowRemoteCursorLockState {
  static String tag(String id) => 'show_remote_cursor_lock_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<RxBool>(tag: key)) {
      final RxBool state = false.obs;
      Get.put<RxBool>(state, tag: key);
    } else {
      Get.find<RxBool>(tag: key).value = false;
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<RxBool>(tag: key)) {
      Get.delete<RxBool>(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class KeyboardEnabledState {
  static String tag(String id) => 'keyboard_enabled_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<RxBool>(tag: key)) {
      // Server side, default true
      final RxBool state = true.obs;
      Get.put<RxBool>(state, tag: key);
    } else {
      Get.find<RxBool>(tag: key).value = true;
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<RxBool>(tag: key)) {
      Get.delete<RxBool>(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class RemoteCursorMovedState {
  static String tag(String id) => 'remote_cursor_moved_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<RxBool>(tag: key)) {
      final RxBool state = false.obs;
      Get.put<RxBool>(state, tag: key);
    } else {
      Get.find<RxBool>(tag: key).value = false;
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<RxBool>(tag: key)) {
      Get.delete<RxBool>(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class RemoteCountState {
  static String tag() => 'remote_count_';

  static void init() {
    final key = tag();
    if (!Get.isRegistered<RxInt>(tag: key)) {
      final RxInt state = 1.obs;
      Get.put<RxInt>(state, tag: key);
    } else {
      Get.find<RxInt>(tag: key).value = 1;
    }
  }

  static void delete() {
    final key = tag();
    if (Get.isRegistered<RxInt>(tag: key)) {
      Get.delete<RxInt>(tag: key);
    }
  }

  static RxInt find() => Get.find<RxInt>(tag: tag());
}

class PeerBoolOption {
  static String tag(String id, String opt) => 'peer_{$opt}_$id';

  static void init(String id, String opt, bool Function() init_getter) {
    final key = tag(id, opt);
    if (!Get.isRegistered<RxBool>(tag: key)) {
      final RxBool value = RxBool(init_getter());
      Get.put<RxBool>(value, tag: key);
    } else {
      Get.find<RxBool>(tag: key).value = init_getter();
    }
  }

  static void delete(String id, String opt) {
    final key = tag(id, opt);
    if (Get.isRegistered<RxBool>(tag: key)) {
      Get.delete<RxBool>(tag: key);
    }
  }

  static RxBool find(String id, String opt) =>
      Get.find<RxBool>(tag: tag(id, opt));
}

class PeerStringOption {
  static String tag(String id, String opt) => 'peer_{$opt}_$id';

  static void init(String id, String opt, String Function() init_getter) {
    final key = tag(id, opt);
    if (!Get.isRegistered<RxString>(tag: key)) {
      final RxString value = RxString(init_getter());
      Get.put<RxString>(value, tag: key);
    } else {
      Get.find<RxString>(tag: key).value = init_getter();
    }
  }

  static void delete(String id, String opt) {
    final key = tag(id, opt);
    if (Get.isRegistered<RxString>(tag: key)) {
      Get.delete<RxString>(tag: key);
    }
  }

  static RxString find(String id, String opt) =>
      Get.find<RxString>(tag: tag(id, opt));
}

class UnreadChatCountState {
  static String tag(id) => 'unread_chat_count_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<RxInt>(tag: key)) {
      final RxInt state = RxInt(0);
      Get.put<RxInt>(state, tag: key);
    } else {
      Get.find<RxInt>(tag: key).value = 0;
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<RxInt>(tag: key)) {
      Get.delete<RxInt>(tag: key);
    }
  }

  static RxInt find(String id) => Get.find<RxInt>(tag: tag(id));
}
