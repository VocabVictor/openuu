import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';

import '../../consts.dart';
import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'globals.dart';

Widget loadPowered(BuildContext context) {
  if (bind.mainGetBuildinOption(key: "hide-powered-by-me") == 'Y') {
    return SizedBox.shrink();
  }
  return MouseRegion(
    cursor: SystemMouseCursors.click,
    child: GestureDetector(
      child: Opacity(
          opacity: 0.5,
          child: Text(
            translate("powered_by_me"),
            overflow: TextOverflow.clip,
            style: Theme.of(context)
                .textTheme
                .bodySmall
                ?.copyWith(fontSize: 9, decoration: TextDecoration.underline),
          )),
    ),
  ).marginOnly(top: 6);
}

const _kDefaultLogoAsset = 'assets/logo.png';

const _kLightLogoAsset = 'assets/logo_light.png';

const _kDarkLogoAsset = 'assets/logo_dark.png';

List<String> _logoAssetCandidatesForBrightness(Brightness brightness) {
  return brightness == Brightness.dark
      ? [_kDarkLogoAsset, _kDefaultLogoAsset]
      : [_kLightLogoAsset, _kDefaultLogoAsset];
}

Future<String?> _resolveLogoAsset(Brightness brightness) async {
  for (final asset in _logoAssetCandidatesForBrightness(brightness)) {
    try {
      await rootBundle.load(asset);
      return asset;
    } on FlutterError {
      continue;
    }
  }
  return null;
}

class _Logo extends StatefulWidget {
  const _Logo();

  @override
  State<_Logo> createState() => _LogoState();
}

class _LogoState extends State<_Logo> {
  final Map<Brightness, Future<String?>> _logoFutures = {};

  Future<String?> _logoFutureFor(Brightness brightness) {
    return _logoFutures.putIfAbsent(
      brightness,
      () => _resolveLogoAsset(brightness),
    );
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<String?>(
      future: _logoFutureFor(Theme.of(context).brightness),
      builder: (BuildContext context, AsyncSnapshot<String?> snapshot) {
        final asset = snapshot.data;
        if (asset != null) {
          final image = Image.asset(
            asset,
            fit: BoxFit.contain,
            errorBuilder: (ctx, error, stackTrace) {
              return Container();
            },
          );
          return Container(
            constraints: BoxConstraints(maxWidth: 300, maxHeight: 60),
            child: image,
          ).marginOnly(left: 12, right: 12, top: 12);
        }
        return const Offstage();
      },
    );
  }
}

// max 300 x 60
Widget loadLogo() => const _Logo();

Widget loadIcon(double size) {
  return Image.asset('assets/icon.png',
      width: size,
      height: size,
      errorBuilder: (ctx, error, stackTrace) => SvgPicture.asset(
            'assets/icon.svg',
            width: size,
            height: size,
          ));
}

var imcomingOnlyHomeSize = Size(280, 300);

Size getIncomingOnlyHomeSize() {
  final magicWidth = isWindows ? 11.0 : 2.0;
  final magicHeight = 10.0;
  return imcomingOnlyHomeSize +
      Offset(magicWidth, kDesktopRemoteTabBarHeight + magicHeight);
}

Size getIncomingOnlySettingsSize() {
  return Size(768, 600);
}

bool isInHomePage() {
  final controller = Get.find<DesktopTabController>();
  return controller.state.value.selected == 0;
}

Widget _buildPresetPasswordWarning() {
  if (bind.mainGetBuildinOption(key: kOptionRemovePresetPasswordWarning) !=
      'N') {
    return SizedBox.shrink();
  }
  return Container(
    color: Colors.yellow,
    child: Column(
      children: [
        Align(
            child: Text(
          translate("Security Alert"),
          style: TextStyle(
            color: Colors.red,
            fontSize:
                18, // https://github.com/rustdesk/rustdesk-server-pro/issues/261
            fontWeight: FontWeight.bold,
          ),
        )).paddingOnly(bottom: 8),
        Text(
          translate("preset_password_warning"),
          style: TextStyle(color: Colors.red),
        )
      ],
    ).paddingAll(8),
  ); // Show a warning message if the Future completed with true
}

Widget buildPresetPasswordWarningMobile() {
  if (bind.isPresetPasswordMobileOnly()) {
    return _buildPresetPasswordWarning();
  } else {
    return SizedBox.shrink();
  }
}

Widget buildPresetPasswordWarning() {
  return FutureBuilder<bool>(
    future: bind.isPresetPassword(),
    builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
      if (snapshot.connectionState == ConnectionState.waiting) {
        return CircularProgressIndicator(); // Show a loading spinner while waiting for the Future to complete
      } else if (snapshot.hasError) {
        return Text(
            'Error: ${snapshot.error}'); // Show an error message if the Future completed with an error
      } else if (snapshot.hasData && snapshot.data == true) {
        return _buildPresetPasswordWarning();
      } else {
        return SizedBox
            .shrink(); // Show nothing if the Future completed with false or null
      }
    },
  );
}
