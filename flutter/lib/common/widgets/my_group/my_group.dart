import 'dart:math';

import 'package:flutter/material.dart';
import 'package:openuu/common/hbbs/hbbs.dart';
import 'package:openuu/common/widgets/login.dart';
import 'package:openuu/common/widgets/peers_view.dart';
import 'package:openuu/models/state_model.dart';
import 'package:get/get.dart';

import '../../../common.dart';
part 'my_group_state.dart';

class MyGroup extends StatefulWidget {
  final EdgeInsets? menuPadding;
  const MyGroup({Key? key, this.menuPadding}) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _MyGroupState();
  }
}
