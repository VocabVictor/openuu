part of 'shared_state.dart';

class PrivacyModeState {
  static String tag(String id) => 'privacy_mode_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<RxString>(tag: key)) {
      final RxString state = ''.obs;
      Get.put<RxString>(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<RxString>(tag: key)) {
      Get.delete<RxString>(tag: key);
    } else {
      Get.find<RxString>(tag: key).value = '';
    }
  }

  static RxString find(String id) => Get.find<RxString>(tag: tag(id));
}

class BlockInputState {
  static String tag(String id) => 'block_input_$id';

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

class CurrentDisplayState {
  static String tag(String id) => 'current_display_$id';

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

class ConnectionType {
  final Rx<String> _secure = kInvalidValueStr.obs;
  final Rx<String> _direct = kInvalidValueStr.obs;
  final Rx<String> _stream_type = kInvalidValueStr.obs;

  Rx<String> get secure => _secure;
  Rx<String> get direct => _direct;
  Rx<String> get stream_type => _stream_type;

  static String get strSecure => 'secure';
  static String get strInsecure => 'insecure';
  static String get strDirect => '';
  static String get strIndirect => '_relay';

  void setSecure(bool v) {
    _secure.value = v ? strSecure : strInsecure;
  }

  void setDirect(bool v) {
    _direct.value = v ? strDirect : strIndirect;
  }

  void setStreamType(String v) {
    _stream_type.value = v;
  }

  bool isValid() {
    return _secure.value != kInvalidValueStr &&
        _direct.value != kInvalidValueStr &&
        _stream_type.value != kInvalidValueStr;
  }
}

class ConnectionTypeState {
  static String tag(String id) => 'connection_type_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<ConnectionType>(tag: key)) {
      final ConnectionType collectionType = ConnectionType();
      Get.put<ConnectionType>(collectionType, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<ConnectionType>(tag: key)) {
      Get.delete<ConnectionType>(tag: key);
    }
  }

  static ConnectionType find(String id) =>
      Get.find<ConnectionType>(tag: tag(id));
}

class FingerprintState {
  static String tag(String id) => 'fingerprint_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered<RxString>(tag: key)) {
      final RxString state = ''.obs;
      Get.put<RxString>(state, tag: key);
    } else {
      Get.find<RxString>(tag: key).value = '';
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered<RxString>(tag: key)) {
      Get.delete<RxString>(tag: key);
    }
  }

  static RxString find(String id) => Get.find<RxString>(tag: tag(id));
}

class ShowRemoteCursorState {
  static String tag(String id) => 'show_remote_cursor_$id';

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
