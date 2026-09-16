import 'native.dart';
import 'package:openuu/generated_bridge.dart';

final platformFFI = PlatformFFI.instance;
final localeName = PlatformFFI.localeName;

RustdeskImpl get bind => platformFFI.ffiBind;

String ffiGetByName(String name, [String arg = '']) {
  return PlatformFFI.getByName(name, arg);
}

void ffiSetByName(String name, [String value = '']) {
  PlatformFFI.setByName(name, value);
}
