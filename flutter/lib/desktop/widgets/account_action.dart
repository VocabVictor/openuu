import 'package:flutter/material.dart';
import 'package:openuu/common.dart';
import 'package:openuu/common/widgets/login.dart';
import 'package:openuu/consts.dart';
import 'package:openuu/desktop/pages/welcome.dart';
import 'package:get/get.dart';
import 'package:openuu/models/user.dart';

/// Account entry for the main title bar: a sign-in icon when signed out, the
/// user's name with a sign-out menu when signed in.
class AccountAction extends StatelessWidget {
  const AccountAction({super.key});

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      final user = gFFI.userModel;
      if (!user.isLogin) {
        return TextButton(
          onPressed: () => loginDialog(),
          style: TextButton.styleFrom(
              foregroundColor: DesktopWelcomePage.blue,
              padding: const EdgeInsets.symmetric(horizontal: 12),
              minimumSize: const Size(0, 28)),
          child: Text(translate('Login'), style: const TextStyle(fontSize: 13)),
        );
      }
      final name = user.displayNameOrUserName;
      final initial = name.isEmpty ? '?' : name.characters.first.toUpperCase();
      return PopupMenuButton<String>(
        tooltip: name,
        offset: const Offset(0, kDesktopRemoteTabBarHeight),
        onSelected: (_) => user.logOut(),
        itemBuilder: (_) => [
          PopupMenuItem(value: 'logout', child: Text(translate('Logout'))),
        ],
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            CircleAvatar(
                radius: 9,
                backgroundColor: DesktopWelcomePage.blue,
                child: Text(initial,
                    style: const TextStyle(fontSize: 10, color: Colors.white))),
            const SizedBox(width: 6),
            ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 120),
                child: Text(name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 12))),
            const Icon(Icons.expand_more, size: 14),
          ]),
        ),
      );
    });
  }
}
