import 'package:flutter_hbb/common/widgets/brand_icon.dart';
import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/user_model.dart';
import 'package:get/get.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../common.dart';
import '.././dialog.dart';
import '.././oidc_auth_status.dart';
part 'widget_op_auth.dart';
part 'widget_op.dart';
part 'oidc.dart';
part 'login_dialog.dart';
part 'login_widgets.dart';
part 'dialogs.dart';

const kOpSvgList = [
  'github',
  'gitlab',
  'google',
  'apple',
  'okta',
  'facebook',
  'azure',
  'auth0',
  'microsoft'
];
const _requestingAccountAuth = 'Requesting account auth';
const _waitingAccountAuth = 'Waiting account auth';
