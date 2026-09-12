part of 'chat_model.dart';

extension ChatModelMessages on ChatModel {
  changeCurrentKey(MessageKey key) {
    updateConnIdOfKey(key);
    String? peerName;
    if (key.connId == ChatModel.clientModeID) {
      peerName = parent.target?.ffiModel.pi.username;
    } else {
      peerName = parent.target?.serverModel.clients
          .firstWhereOrNull((client) => client.peerId == key.peerId)
          ?.name;
    }
    if (!_messages.containsKey(key)) {
      final chatUser = ChatUser(
        id: key.peerId,
        firstName: peerName,
      );
      _messages[key] = MessageBody(chatUser, []);
    } else {
      if (peerName != null && peerName.isNotEmpty) {
        _messages[key]?.chatUser.firstName = peerName;
      }
    }
    _currentKey = key;
    _notify();
    mobileClearClientUnread(key.connId);
  }

  receive(int id, String text) async {
    final session = parent.target;
    if (session == null) {
      debugPrint("Failed to receive msg, session state is null");
      return;
    }
    if (text.isEmpty) return;
    if (desktopType == DesktopType.cm) {
      await showCmWindow();
    }
    String? peerId;
    if (id == ChatModel.clientModeID) {
      peerId = session.id;
    } else {
      peerId = session.serverModel.clients
          .firstWhereOrNull((e) => e.id == id)
          ?.peerId;
    }
    if (peerId == null) {
      debugPrint("Failed to receive msg, peerId is null");
      return;
    }

    final messagekey = MessageKey(peerId, id);

    // mobile: first message show overlay icon
    if (!isDesktop && chatIconOverlayEntry == null) {
      showChatIconOverlay();
    }
    // show chat page
    await showChatPage(messagekey);
    late final ChatUser chatUser;
    if (id == ChatModel.clientModeID) {
      chatUser = ChatUser(
        firstName: session.ffiModel.pi.username,
        id: peerId,
      );

      if (isDesktop) {
        if (Get.isRegistered<DesktopTabController>()) {
          DesktopTabController tabController = Get.find<DesktopTabController>();
          var index = tabController.state.value.tabs
              .indexWhere((e) => e.key == session.id);
          final notSelected =
              index >= 0 && tabController.state.value.selected != index;
          // minisized: top and switch tab
          // not minisized: add count
          if (await WindowController.fromWindowId(stateGlobal.windowId)
              .isMinimized()) {
            windowOnTop(stateGlobal.windowId);
            if (notSelected) {
              tabController.jumpTo(index);
            }
          } else {
            if (notSelected) {
              UnreadChatCountState.find(peerId).value += 1;
            }
          }
        }
      }
    } else {
      final client = session.serverModel.clients
          .firstWhereOrNull((client) => client.id == id);
      if (client == null) {
        debugPrint("Failed to receive msg, client is null");
        return;
      }
      if (isDesktop) {
        windowOnTop(null);
        // disable auto jumpTo other tab when hasFocus, and mark unread message
        final currentSelectedTab =
            session.serverModel.tabController.state.value.selectedTabInfo;
        if (currentSelectedTab.key != id.toString() && inputNode.hasFocus) {
          client.unreadChatMessageCount.value += 1;
        } else {
          parent.target?.serverModel.jumpTo(id);
        }
      } else {
        if (HomePage.homeKey.currentState?.isChatPageCurrentTab != true ||
            _currentKey != messagekey) {
          client.unreadChatMessageCount.value += 1;
          mobileUpdateUnreadSum();
        }
      }
      chatUser = ChatUser(id: client.peerId, firstName: client.name);
    }
    insertMessage(messagekey,
        ChatMessage(text: text, user: chatUser, createdAt: DateTime.now()));
    if (id == ChatModel.clientModeID || _currentKey.peerId.isEmpty) {
      // client or invalid
      _currentKey = messagekey;
      mobileClearClientUnread(messagekey.connId);
    }
    latestReceivedKey = messagekey;
    _notify();
  }

  send(ChatMessage message) {
    String trimmedText = message.text.trim();
    if (trimmedText.isEmpty) {
      return;
    }
    message.text = trimmedText;
    insertMessage(_currentKey, message);
    if (_currentKey.connId == ChatModel.clientModeID && parent.target != null) {
      bind.sessionSendChat(sessionId: sessionId, text: message.text);
    } else {
      bind.cmSendChat(connId: _currentKey.connId, msg: message.text);
    }

    _notify();
    inputNode.requestFocus();
  }

  insertMessage(MessageKey key, ChatMessage message) {
    updateConnIdOfKey(key);
    if (!_messages.containsKey(key)) {
      _messages[key] = MessageBody(message.user, []);
    }
    _messages[key]?.insert(message);
  }

  updateConnIdOfKey(MessageKey key) {
    if (_messages.keys
            .toList()
            .firstWhereOrNull((e) => e == key && e.connId != key.connId) !=
        null) {
      final value = _messages.remove(key);
      if (value != null) {
        _messages[key] = value;
      }
    }
    if (_currentKey == key || _currentKey.peerId.isEmpty) {
      _currentKey = key; // hash != assign
    }
  }

  void mobileUpdateUnreadSum() {
    if (!isMobile) return;
    var sum = 0;
    parent.target?.serverModel.clients
        .map((e) => sum += e.unreadChatMessageCount.value)
        .toList();
    Future.delayed(Duration.zero, () {
      mobileUnreadSum.value = sum;
    });
  }

  void mobileClearClientUnread(int id) {
    if (!isMobile) return;
    final client = parent.target?.serverModel.clients
        .firstWhereOrNull((client) => client.id == id);
    if (client != null) {
      Future.delayed(Duration.zero, () {
        client.unreadChatMessageCount.value = 0;
        mobileUpdateUnreadSum();
      });
    }
  }
}
