import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_welcome_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:get/get.dart';
import 'package:flutter_hbb/models/user_model.dart';

/// Account entry for the main title bar: a sign-in icon when signed out, the
/// user's name with a sign-out menu when signed in.
class AccountAction extends StatelessWidget {
  const AccountAction({super.key});

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      final user = gFFI.userModel;
      if (!user.isLogin) {
        return ActionIcon(
          message: 'Login',
          icon: Icons.person_outline,
          onTap: () => loginDialog(),
          isClose: false,
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
