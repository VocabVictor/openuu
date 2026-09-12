part of 'terminal_page.dart';

extension _TerminalPageKeyboard on _TerminalPageState {
  Widget _buildFloatingKeyboard() {
    return AnimatedPositioned(
      duration: const Duration(milliseconds: 200),
      left: 0,
      right: 0,
      bottom: _sysKeyboardHeight,
      child: Container(
        key: _keyboardKey,
        color: Theme.of(context).scaffoldBackgroundColor,
        padding: EdgeInsets.zero,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            // Row 1 follows the latest reviewed PR layout.
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: _buildKeyboardKeyButtons(terminalKeyboardRow1Keys),
            ),
            // Row 2 ends with the full-width Row3 collapse/expand toggle.
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                ..._buildKeyboardKeyButtons(terminalKeyboardRow2Keys),
                const SizedBox(width: terminalKeyboardKeySpacing),
                _buildCollapseButton(),
              ],
            ),
            // Row 3 restores paging keys and trailing alignment placeholders.
            if (_row3Expanded)
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  ..._buildKeyboardKeyButtons(terminalKeyboardRow3Keys),
                  for (var i = 0;
                      i < terminalKeyboardRow3TrailingPlaceholderCount;
                      i++) ...[
                    const SizedBox(width: terminalKeyboardKeySpacing),
                    const SizedBox(width: terminalKeyboardKeyWidth),
                  ],
                ],
              ),
          ],
        ),
      ),
    );
  }

  // Ctrl toggle button with highlighted locked state
  Widget _buildCtrlKeyButton() {
    return _buildModifierToggleButton(
      text: 'Ctrl',
      semanticsLabel: 'Ctrl',
      isLocked: _ctrlLocked,
      onPressed: () => _setState(() => _ctrlLocked = !_ctrlLocked),
    );
  }

  // Alt toggle button with highlighted locked state
  Widget _buildAltKeyButton() {
    return _buildModifierToggleButton(
      text: 'Alt',
      semanticsLabel: 'Alt',
      isLocked: _altLocked,
      onPressed: () => _setState(() => _altLocked = !_altLocked),
    );
  }

  // Collapse/expand toggle button for Row3
  void _toggleRow3Expanded() {
    final willExpand = !_row3Expanded;
    final shouldClearModifiers = shouldClearTerminalModifiersWhenRow3Collapses(
      wasExpanded: _row3Expanded,
      willExpand: willExpand,
      ctrlLocked: _ctrlLocked,
      altLocked: _altLocked,
    );
    _setState(() {
      _row3Expanded = willExpand;
      if (shouldClearModifiers) {
        _ctrlLocked = false;
        _altLocked = false;
      }
    });
    mainSetLocalBoolOption(kOptionShowTerminalCtrlKeys, willExpand);

    // The floating keyboard height changes after Row3 is inserted/removed.
    // Re-measure on the next frame so terminal padding uses the new height.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_showTerminalExtraKeys) return;
      _setState(() {
        _updateKeyboardHeight();
      });
    });
  }

  Widget _buildCollapseButton() {
    return Semantics(
      label: translate('Show terminal extra keys'),
      toggled: _row3Expanded,
      child: ElevatedButton(
        onPressed: _toggleRow3Expanded,
        child: Text(_row3Expanded ? '∧' : '∨'),
        style: ElevatedButton.styleFrom(
          minimumSize: const Size(terminalKeyboardKeyWidth, 32),
          padding: EdgeInsets.zero,
          textStyle: const TextStyle(fontSize: 12),
          backgroundColor:
              Theme.of(context).colorScheme.surfaceContainerHighest,
          foregroundColor: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }

  /// Builds a fixed-width key sequence with the reviewed 2dp spacing.
  List<Widget> _buildKeyboardKeyButtons(List<String> labels) {
    return [
      for (var i = 0; i < labels.length; i++) ...[
        _buildKeyButton(labels[i]),
        if (i < labels.length - 1)
          const SizedBox(width: terminalKeyboardKeySpacing),
      ],
    ];
  }

  /// Build a modifier toggle button (Ctrl/Alt) with one-shot behavior.
  /// When [isLocked] is true, the button highlights in blue and the next
  /// single-character input is mapped to its modified equivalent.
  Widget _buildModifierToggleButton({
    required String text,
    required String semanticsLabel,
    required bool isLocked,
    required VoidCallback onPressed,
  }) {
    return Semantics(
      // Ctrl and Alt are technical key names and intentionally stay unchanged.
      label: semanticsLabel,
      toggled: isLocked,
      child: ElevatedButton(
        onPressed: onPressed,
        child: Text(text),
        style: ElevatedButton.styleFrom(
          minimumSize: const Size(terminalKeyboardKeyWidth, 32),
          padding: EdgeInsets.zero,
          textStyle: const TextStyle(fontSize: 12),
          backgroundColor: isLocked
              ? Colors.blue
              : Theme.of(context).colorScheme.surfaceContainerHighest,
          foregroundColor: isLocked
              ? Colors.white
              : Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }

  Widget _buildKeyButton(String label) {
    if (label == 'Ctrl') return _buildCtrlKeyButton();
    if (label == 'Alt') return _buildAltKeyButton();

    return ElevatedButton(
      onPressed: () {
        _sendKeyToTerminal(label);
      },
      child: Text(label),
      style: ElevatedButton.styleFrom(
        minimumSize: const Size(terminalKeyboardKeyWidth, 32),
        padding: EdgeInsets.zero,
        textStyle: const TextStyle(fontSize: 12),
        backgroundColor:
            Theme.of(context).colorScheme.surfaceContainerHighest,
        foregroundColor: Theme.of(context).colorScheme.onSurfaceVariant,
      ),
    );
  }

  void _sendKeyToTerminal(String key) {
    String send;

    switch (key) {
      case 'Esc':
        send = '\x1B';
        break;
      case 'Tab':
        send = '\t';
        break;
      case 'Ctrl+C':
        send = '\x03';
        break;

      case '↑':
        send = '\x1B[A';
        break;
      case '↓':
        send = '\x1B[B';
        break;
      case '→':
        send = '\x1B[C';
        break;
      case '←':
        send = '\x1B[D';
        break;

      case 'Home':
        send = '\x1B[H';
        break;
      case 'End':
        send = '\x1B[F';
        break;
      case 'PgUp':
        send = '\x1B[5~';
        break;
      case 'PgDn':
        send = '\x1B[6~';
        break;

      default:
        send = key;
        break;
    }

    _terminalModel.sendVirtualKey(send);
  }

  // https://github.com/TerminalStudio/xterm.dart/issues/42#issuecomment-877495472
  // https://github.com/TerminalStudio/xterm.dart/issues/198#issuecomment-2526548458
  TerminalStyle _getTerminalStyle() {
    return isWeb
        ? TerminalStyle(
            fontFamily: _robotoMonoFontFamily,
            fontSize: 14,
          )
        : const TerminalStyle();
  }
}
