part of 'hbbs.dart';

// to-do: The UserPayload does not contain all the fields of the user.
// Is all the fields of the user needed?
class UserPayload {
  String name = '';
  String displayName = '';
  String avatar = '';
  String email = '';
  String note = '';
  String? verifier;
  UserStatus status;
  bool isAdmin = false;

  UserPayload.fromJson(Map<String, dynamic> json)
      : name = json['name'] ?? '',
        displayName = json['display_name'] ?? '',
        avatar = json['avatar'] ?? '',
        email = json['email'] ?? '',
        note = json['note'] ?? '',
        verifier = json['verifier'],
        status = json['status'] == 0
            ? UserStatus.kDisabled
            : json['status'] == -1
                ? UserStatus.kUnverified
                : UserStatus.kNormal,
        isAdmin = json['is_admin'] == true;

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> map = {
      'name': name,
      'display_name': displayName,
      'avatar': avatar,
      'status': status == UserStatus.kDisabled
          ? 0
          : status == UserStatus.kUnverified
              ? -1
              : 1,
    };
    return map;
  }

  Map<String, dynamic> toGroupCacheJson() {
    final Map<String, dynamic> map = {
      'name': name,
      'display_name': displayName,
    };
    return map;
  }

  String get displayNameOrName {
    return displayName.trim().isEmpty ? name : displayName;
  }
}

class PeerPayload {
  String id = '';
  Map<String, dynamic> info = {};
  int? status;
  String user = '';
  String user_name = '';
  String? device_group_name;
  String note = '';

  PeerPayload.fromJson(Map<String, dynamic> json)
      : id = json['id'] ?? '',
        info = (json['info'] is Map<String, dynamic>) ? json['info'] : {},
        status = json['status'],
        user = json['user'] ?? '',
        user_name = json['user_name'] ?? '',
        device_group_name = json['device_group_name'] ?? '',
        note = json['note'] ?? '';

  static Peer toPeer(PeerPayload p) {
    return Peer.fromJson({
      "id": p.id,
      'loginName': p.user_name,
      "username": p.info['username'] ?? '',
      "platform": _platform(p.info['os']),
      "hostname": p.info['device_name'],
      "device_group_name": p.device_group_name,
      "note": p.note,
    });
  }

  static String? _platform(dynamic field) {
    if (field == null) {
      return null;
    }
    final fieldStr = field.toString();
    List<String> list = fieldStr.split(' / ');
    if (list.isEmpty) return null;
    final os = list[0];
    switch (os.toLowerCase()) {
      case 'windows':
        return kPeerPlatformWindows;
      case 'linux':
        return kPeerPlatformLinux;
      case 'macos':
        return kPeerPlatformMacOS;
      case 'android':
        return kPeerPlatformAndroid;
      default:
        if (fieldStr.toLowerCase().contains('linux')) {
          return kPeerPlatformLinux;
        }
        return null;
    }
  }
}
