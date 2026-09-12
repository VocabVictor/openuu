import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

/// Shared product mark for title bars, account dialogs and the About page.
class BrandIcon extends StatelessWidget {
  final double size;
  const BrandIcon({super.key, this.size = 24});

  @override
  Widget build(BuildContext context) => SvgPicture.asset(
        'assets/icon.svg',
        width: size,
        height: size,
        semanticsLabel: 'OpenUU',
      );
}
