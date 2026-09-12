import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';

void main() {
  group('shouldHandleTerminalPasteShortcut', () {
    test('handles only Ctrl+Shift+V on Linux with a virtual lock', () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.linux,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: true,
          modifierLockActive: true,
        ),
        isTrue,
      );
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.linux,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isFalse,
      );
    });

    test(
        'keeps default xterm paste behavior when virtual modifiers are inactive',
        () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: false,
        ),
        isFalse,
      );
    });

    test('handles Ctrl+V and Meta+V when a virtual modifier lock is active',
        () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isTrue,
      );
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.macOS,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: false,
          metaPressed: true,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isTrue,
      );
    });

    test('handles paste shortcut repeats while a virtual lock is active', () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: false,
          isKeyRepeat: true,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isTrue,
      );
    });

    test('ignores key-up and unmodified V events', () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: false,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isFalse,
      );
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: false,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isFalse,
      );
    });

    test('ignores paste shortcuts with extra modifiers', () {
      for (final state in [
        (control: true, meta: false, alt: true, shift: false),
        (control: true, meta: false, alt: false, shift: true),
        (control: false, meta: true, alt: false, shift: true),
        (control: true, meta: true, alt: false, shift: false),
      ]) {
        expect(
          shouldHandleTerminalPasteShortcut(
            platform: TargetPlatform.windows,
            logicalKey: LogicalKeyboardKey.keyV,
            isKeyDown: true,
            isKeyRepeat: false,
            controlPressed: state.control,
            metaPressed: state.meta,
            altPressed: state.alt,
            shiftPressed: state.shift,
            modifierLockActive: true,
          ),
          isFalse,
        );
      }
    });

    test('ignores non-V key events', () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyC,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isFalse,
      );
    });
  });

  group('shouldClearTerminalModifiersWhenRow3Collapses', () {
    test('clears visible modifier state when expanded row is collapsed', () {
      expect(
        shouldClearTerminalModifiersWhenRow3Collapses(
          wasExpanded: true,
          willExpand: false,
          ctrlLocked: true,
          altLocked: false,
        ),
        isTrue,
      );
    });

    test('does not clear modifiers when row expands', () {
      expect(
        shouldClearTerminalModifiersWhenRow3Collapses(
          wasExpanded: false,
          willExpand: true,
          ctrlLocked: true,
          altLocked: true,
        ),
        isFalse,
      );
    });

    test('clears Alt state when expanded row is collapsed', () {
      expect(
        shouldClearTerminalModifiersWhenRow3Collapses(
          wasExpanded: true,
          willExpand: false,
          ctrlLocked: false,
          altLocked: true,
        ),
        isTrue,
      );
    });
  });
}
